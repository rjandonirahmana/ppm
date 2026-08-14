//! repository/permits.rs — Query pengajuan & antrean izin.
//!
//! SATU tahap: wali kelas KBM santri memutuskan, titik. Peran lama beserta
//! seluruh kolomnya dihapus (migrasi 84 & 86).
//!
//! Izin disimpan sebagai RENTANG WAKTU (`start_time`/`end_time`, keduanya
//! TIMESTAMP sejak migrasi 86): dari saat santri keluar sampai saat ia kembali.
//! Izin sehari penuh = 00:00 → 23:59:59 pada tanggal yang sama, bukan keadaan
//! khusus. Semua pertanyaan "kelas ini terlewat atau tidak" dijawab dengan satu
//! aturan: apakah jam kelasnya beririsan dengan rentang itu.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use deadpool_postgres::Pool;

/// Target notifikasi izin: penyetuju kelas UTAMA santri (wali kelas selalu;
/// Wali kelas kelas tujuan izin.
/// Wali kelas penerima notifikasi izin (nama & HP).
pub struct PermitNotifyTargets {
    pub student_name: String,
    pub wali_phone: Option<String>,
}

/// Wali kelas yang harus diberi tahu bahwa ada izin baru di kelasnya.
pub async fn permit_notify_targets(
    pool: &Pool,
    student_id: i64,
    class_id: Option<i64>,
) -> Result<Option<PermitNotifyTargets>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT u.full_name, w.phone_number \
           FROM users u \
           LEFT JOIN classes tc ON tc.id = $2 \
           {kelas} \
           LEFT JOIN users w ON w.id = COALESCE(tc.wali_kelas_id, cl.wali_kelas_id) \
          WHERE u.id = $1",
        kelas = super::kelas_utama_lateral("u.id"),
    );
    let row = c
        .query_opt(&sql, &[&student_id, &class_id])
        .await
        .context("permit_notify_targets")?;
    Ok(row.map(|r| PermitNotifyTargets {
        student_name: r.get(0),
        wali_phone: r.get(1),
    }))
}

pub struct KelasKbmSantri {
    pub class_id: i64,
    pub class_name: String,
    pub wali_kelas_id: Option<i64>,
    pub wali_name: Option<String>,
}

pub async fn kelas_kbm_santri(pool: &Pool, student_id: i64) -> Result<Option<KelasKbmSantri>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cl.id, cl.name, cl.wali_kelas_id, w.full_name \
               FROM class_participants cp \
               JOIN classes cl ON cl.id = cp.class_id AND cl.category = 'kbm' \
               LEFT JOIN users w ON w.id = cl.wali_kelas_id \
              WHERE cp.user_id = $1 \
              ORDER BY cl.id LIMIT 1",
            &[&student_id],
        )
        .await
        .context("kelas_kbm_santri")?;
    Ok(row.map(|r| KelasKbmSantri {
        class_id: r.get(0),
        class_name: r.get(1),
        wali_kelas_id: r.get(2),
        wali_name: r.get(3),
    }))
}

/// Satu kelas yang dilewati selama rentang izin, beserta penanggung jawabnya.
/// Dipakai `service::permits::auto_create_permits_per_wali` untuk memecah satu
/// pengajuan jadi beberapa `permit_requests` (satu per wali kelas unik).
/// Satu (kelas, jadwal) calon terdampak izin. Satu kelas bisa muncul beberapa
/// kali bila jadwalnya lebih dari satu — penyaring recurrence di service yang
/// memutuskan mana yang benar-benar jatuh di rentang izin.
pub struct AffectedClass {
    pub class_id: i64,
    pub class_name: String,
    pub wali_kelas_id: Option<i64>,
    pub wali_name: Option<String>,
    /// once|daily|weekly|monthly|custom — ditafsirkan `dates_in_range`.
    pub recurrence_type: String,
    pub sched_start: NaiveDate,
    pub sched_end: Option<NaiveDate>,
    /// Tanggal manual untuk recurrence 'custom' ("YYYY-MM-DD"). Kosong untuk
    /// pola lain. WAJIB ikut: seluruh jadwal KBM di produksi memakai custom,
    /// dan tanpa daftar ini pola-nya tak bisa diuji sama sekali.
    pub custom_dates: Vec<String>,
    /// Jam jadwal ini. Dibawa keluar karena penyaringan jam TAK BISA dilakukan
    /// di SQL: tanggal mana saja yang dihasilkan sebuah pola perulangan baru
    /// diketahui setelah `dates_in_range` menghitungnya di Rust. Aturan
    /// jamnya sendiri seragam — lihat `service::permits::tanggal_izin`.
    pub jam_mulai: NaiveTime,
    pub jam_selesai: NaiveTime,
}

/// Kelas yang jadwalnya BERSINGGUNGAN dengan rentang izin [start, end] dan
/// santri terdaftar sebagai peserta. Terurut per wali kelas agar pemanggil bisa
/// mengelompokkan tanpa sorting ulang.
///
/// Catatan: memakai `class_schedules` (jadwal periodik) — sesi yang sudah
/// dimaterialisasi (`class_sessions`) tak dipakai karena izin bisa diajukan
/// untuk tanggal yang sesinya belum di-generate.
pub async fn affected_classes(
    pool: &Pool,
    student_id: i64,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<AffectedClass>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            // Satu baris PER JADWAL, bukan per kelas: pola recurrence-nya
            // dibawa keluar supaya pemanggil bisa menguji apakah jadwal itu
            // benar-benar jatuh di rentang izin. Rentang tanggal berlaku saja
            // tak cukup — jadwal Senin "berlaku" sepanjang semester, jadi izin
            // hari Selasa ikut menyeret kelas yang hari itu tak berlangsung.
            //
            // Recurrence-nya TIDAK dihitung di SQL sini melainkan di Rust
            // (service::kelas::dates_in_range) — satu penafsiran daily/weekly/
            // monthly/once untuk seluruh aplikasi. Dan tak bisa memakai
            // class_sessions seperti jalur RFID: sesi hanya dimaterialisasi 7
            // hari ke depan, sedangkan izin lazim diajukan jauh sebelumnya.
            //
            // `custom_dates` DISARING DI SQL, bukan dikirim utuh lalu difilter
            // di Rust: daftarnya bertambah tiap tahun ajaran dan tak pernah
            // menyusut, sementara yang dibutuhkan hanya tanggal di dalam
            // rentang izin — biasanya beberapa hari. Tanpa saringan ini, satu
            // pratinjau izin menarik seluruh riwayat tanggal tiap jadwal.
            // Perbandingan sebagai TEKS, bukan cast ke date: format ISO
            // "YYYY-MM-DD" berurutan secara leksikografis sama persis dengan
            // urutan tanggalnya, dan satu string cacat di data lama tak
            // menggagalkan seluruh query seperti yang dilakukan `::date`.
            //
            // JAM TIDAK DISARING DI SINI (lihat `AffectedClass::jam_mulai`).
            // Sampai Ags 2026 query ini menyaring `cs.start_time < $jam_selesai
            // AND cs.end_time > $jam_mulai` — memperlakukan jam izin sebagai
            // "berlaku pada SETIAP hari" dalam rentang. Itu keliru untuk izin
            // berhari-hari: santri yang pulang Jumat 14:00 dan kembali Minggu
            // 08:00 tidak "izin 14:00–08:00 tiap hari", ia PERGI — dan seluruh
            // kelas di antara dua titik itu ikut terlewat. Keputusannya kini di
            // Rust, per tanggal (`service::permits::tanggal_izin`).
            "SELECT cl.id, cl.name, cl.wali_kelas_id, w.full_name, \
                    cs.recurrence_type, cs.start_date, cs.end_date, \
                    cs.start_time, cs.end_time, \
                    (SELECT COALESCE(jsonb_agg(d.v), '[]'::jsonb) \
                       FROM jsonb_array_elements_text( \
                              COALESCE(cs.custom_dates, '[]'::jsonb)) AS d(v) \
                      WHERE d.v >= to_char($2::date, 'YYYY-MM-DD') \
                        AND d.v <= to_char($3::date, 'YYYY-MM-DD')) \
             FROM class_schedules cs \
             JOIN classes cl ON cl.id = cs.class_id \
             LEFT JOIN users w ON w.id = cl.wali_kelas_id \
             JOIN class_participants cp ON cp.class_id = cl.id AND cp.user_id = $1 \
             WHERE cs.status = 'active' \
               AND cs.start_date <= $3 \
               AND COALESCE(cs.end_date, $3) >= $2 \
             ORDER BY cl.wali_kelas_id NULLS LAST, cl.name, cs.id",
            &[&student_id, &start_date, &end_date],
        )
        .await
        .context("affected_classes")?;
    Ok(rows
        .into_iter()
        .map(|r| AffectedClass {
            class_id: r.get(0),
            class_name: r.get(1),
            wali_kelas_id: r.get(2),
            wali_name: r.get(3),
            recurrence_type: r.get(4),
            sched_start: r.get(5),
            sched_end: r.get(6),
            jam_mulai: r.get(7),
            jam_selesai: r.get(8),
            custom_dates: r
                .get::<_, Option<serde_json::Value>>(9)
                .and_then(|v| {
                    v.as_array().map(|a| {
                        a.iter().filter_map(|x| x.as_str().map(String::from)).collect()
                    })
                })
                .unwrap_or_default(),
        })
        .collect())
}

/// Dampak beberapa izin sekaligus: `permit_id → (label per kelas, total sesi)`.
///
/// Jamnya dibandingkan sebagai RENTANG (tanggal+jam), sama seperti
/// `materialize_permit_attendance` & auto-alpa — tiga tempat ini harus menjawab
/// hal yang sama, kalau tidak wali kelas melihat "2 sesi terlewat" lalu yang
/// benar-benar tercatat izin ada lima.
///
/// Label berbentuk "kelas lambatan (3 sesi)". Sesi dihitung dari
/// `class_sessions` yang benar-benar ada dalam rentang izin — bukan ditaksir
/// dari pola jadwal — sehingga angka yang dilihat wali kelas adalah kelas yang
/// betul-betul akan kosong.
///
/// Satu query untuk seluruh antrean, bukan satu per baris: halaman izin staf
/// menampilkan puluhan baris sekaligus.
pub async fn dampak_izin(
    pool: &Pool,
    permit_ids: &[i64],
) -> Result<std::collections::HashMap<i64, (Vec<String>, i64)>> {
    if permit_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let c = pool.get().await?;
    let rows = c
        .query(
            // Cakupan dihitung dari IRISAN WAKTU + keanggotaan kelas, bukan
            // dari daftar kelas tersimpan (migrasi 86). Sesi yang jamnya
            // beririsan dengan rentang izin = sesi yang akan kosong.
            "SELECT p.id, cl.name, COUNT(s.id)::bigint \
             FROM permit_requests p \
             JOIN class_participants cp ON cp.user_id = p.user_id \
             JOIN classes cl ON cl.id = cp.class_id \
             JOIN class_sessions s ON s.class_id = cp.class_id \
                  AND s.status <> 'cancelled' \
             JOIN class_schedules sch ON sch.id = s.class_schedule_id \
             WHERE p.id = ANY($1::bigint[]) \
               AND (s.session_date + sch.start_time) < p.end_time \
               AND (s.session_date + sch.end_time) > p.start_time \
             GROUP BY p.id, cl.id, cl.name \
             ORDER BY p.id, cl.name",
            &[&permit_ids],
        )
        .await
        .context("dampak_izin")?;

    let mut out: std::collections::HashMap<i64, (Vec<String>, i64)> =
        std::collections::HashMap::new();
    for r in rows {
        let id: i64 = r.get(0);
        let nama: String = r.get(1);
        let n: i64 = r.get(2);
        let e = out.entry(id).or_default();
        e.0.push(if n > 0 {
            format!("{nama} ({n} sesi)")
        } else {
            nama
        });
        e.1 += n;
    }
    Ok(out)
}

/// Sisipkan SATU baris izin yang ditujukan ke satu kelas + wali kelas tertentu.
///
/// Rentangnya satu potongan waktu utuh: `mulai` = saat santri keluar,
/// `selesai` = saat ia kembali (migrasi 86). Izin sehari penuh diwakili
/// 00:00 → 23:59:59 pada tanggal yang sama — bukan keadaan khusus, cuma rentang
/// yang kebetulan selebar satu hari.
#[allow(clippy::too_many_arguments)]
pub async fn insert_permit(
    pool: &Pool,
    user_id: i64,
    requested_by: i64,
    kind: &str,
    mulai: NaiveDateTime,
    selesai: NaiveDateTime,
    reason: &str,
    class_id: Option<i64>,
    wali_kelas_id: Option<i64>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO permit_requests \
                (user_id, requested_by, type, reason, start_time, end_time, class_id, \
                 wali_kelas_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            &[
                &user_id,
                &requested_by,
                &kind,
                &reason,
                &mulai,
                &selesai,
                &class_id,
                &wali_kelas_id,
            ],
        )
        .await
        .context("insert_permit")?;
    Ok(row.get(0))
}

pub struct PermitRow {
    pub id: i64,
    /// Diajukan ORANG TUA atas nama santri (bukan santri sendiri).
    pub oleh_ortu: bool,
    pub requester_name: String,
    pub kind: String,
    pub mulai: NaiveDateTime,
    pub selesai: NaiveDateTime,
    pub guru_status: String,
    /// Nama kelas yang izin ini tujukan — supaya santri tahu izinnya terpecah
    /// ke mana saja ("Menunggu Wali Kelas — Fiqih Lanjutan").
    pub class_name: Option<String>,
}

/// Satu pengajuan izin, lengkap — dipakai SEMUA peran dengan payload yang sama.
///
/// Satu bentuk untuk santri, orang tua, wali kelas, dan admin: yang berbeda
/// hanya WEWENANGNYA, dan itu dihitung di service. Dua payload berbeda untuk
/// data yang sama cepat atau lambat berbeda isinya.
pub struct PermitDetailRow {
    pub id: i64,
    pub user_id: i64,
    pub student_name: String,
    pub kind: String,
    pub reason: String,
    pub mulai: NaiveDateTime,
    pub selesai: NaiveDateTime,
    pub guru_status: String,
    pub class_name: Option<String>,
    pub wali_kelas_id: Option<i64>,
    pub wali_name: Option<String>,
    /// Siapa yang MENGAJUKAN. Sama dengan `user_id` bila santri sendiri.
    pub requested_by: i64,
    pub requester_name: String,
    /// Peran pengaju — dipakai UI menyebut "diajukan orang tua".
    pub requester_role: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn permit_detail(pool: &Pool, permit_id: i64) -> Result<Option<PermitDetailRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT p.id, p.user_id, u.full_name, p.type, p.reason, \
                p.start_time, p.end_time, p.guru_status, \
                COALESCE(tc.name, cl.name), \
                COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id), \
                w.full_name, p.requested_by, rb.full_name, rb.role, p.created_at \
         FROM permit_requests p \
         JOIN users u ON u.id = p.user_id \
         LEFT JOIN users rb ON rb.id = p.requested_by \
         LEFT JOIN classes tc ON tc.id = p.class_id \
         {kelas} \
         LEFT JOIN users w ON w.id = COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id) \
         WHERE p.id = $1",
        kelas = super::kelas_utama_lateral("p.user_id"),
    );
    let row = c.query_opt(&sql, &[&permit_id]).await.context("permit_detail")?;
    Ok(row.map(|r| PermitDetailRow {
        id: r.get(0),
        user_id: r.get(1),
        student_name: r.get(2),
        kind: r.get(3),
        reason: r.get(4),
        mulai: r.get(5),
        selesai: r.get(6),
        guru_status: r.get(7),
        class_name: r.get(8),
        wali_kelas_id: r.get(9),
        wali_name: r.get(10),
        requested_by: r.get(11),
        requester_name: r.get::<_, Option<String>>(12).unwrap_or_default(),
        requester_role: r.get::<_, Option<String>>(13).unwrap_or_default(),
        created_at: r.get(14),
    }))
}

/// Ubah isi pengajuan izin yang MASIH menunggu keputusan.
///
/// Syaratnya ditegakkan di WHERE, bukan diperiksa lebih dulu lalu ditulis:
///   • `guru_status = 'pending'` — izin yang sudah disetujui/ditolak wali kelas
///     terkunci. Absensinya sudah terlanjur diwujudkan; mengubah tanggalnya
///     setelah itu meninggalkan baris izin di sesi yang tak lagi tercakup.
///   • pengubahnya SANTRI pemilik izin, atau WALI KELAS tujuannya. Orang tua
///     boleh mengajukan, tapi tidak mengubah — begitu izin berjalan, yang
///     berwenang adalah santri dan walinya.
///
/// Return false = tak memenuhi salah satu syarat itu (tanpa membedakan yang
/// mana; pemanggil yang menyusun pesannya).
#[allow(clippy::too_many_arguments)]
pub async fn update_permit(
    pool: &Pool,
    permit_id: i64,
    actor_id: i64,
    kind: &str,
    mulai: NaiveDateTime,
    selesai: NaiveDateTime,
    reason: &str,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p \
                SET type = $3, start_time = $4, end_time = $5, reason = $6 \
              WHERE p.id = $1 \
                AND p.guru_status = 'pending' \
                AND (p.user_id = $2 OR p.wali_kelas_id = $2)",
            &[
                &permit_id,
                &actor_id,
                &kind,
                &mulai,
                &selesai,
                &reason,
            ],
        )
        .await
        .context("update_permit")?;
    Ok(n > 0)
}

pub async fn list_my_permits(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<PermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!(
                "SELECT p.type, p.start_time, p.end_time, p.guru_status, tc.name, \
                    p.id, (p.requested_by <> p.user_id AND rb.role = 'parent'), \
                    COALESCE(rb.full_name, '') \
             FROM permit_requests p \
             LEFT JOIN users rb ON rb.id = p.requested_by \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             {kelas} \
             WHERE p.user_id = $1 ORDER BY p.created_at DESC LIMIT $2",
                kelas = super::kelas_utama_lateral("p.user_id"),
            ),
            &[&user_id, &limit],
        )
        .await
        .context("list_my_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PermitRow {
            kind: r.get(0),
            mulai: r.get(1),
            selesai: r.get(2),
            guru_status: r.get(3),
            class_name: r.get(4),
            id: r.get(5),
            oleh_ortu: r.get(6),
            requester_name: r.get(7),
        })
        .collect())
}

// CATATAN (migrasi 46): tahap konfirmasi ORANG TUA DIHAPUS. Fungsi
// `pending_parent_confirms` & `confirm_parent_permit` ikut dihapus — orang tua
// kini hanya MELIHAT izin anaknya (lihat repository/parents.rs), tak memutus.

// ── Antrean keputusan WALI KELAS (satu-satunya tahap) ────────────────────────

/// Satu baris antrean izin yang menunggu keputusan wali kelas.
pub struct PermitQueueRow {
    pub id: i64,
    pub student_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub kind: String,
    /// Rentang izin — keluar & kembali, lengkap dengan jamnya (migrasi 86).
    pub mulai: NaiveDateTime,
    pub selesai: NaiveDateTime,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}

/// Antrean wali kelas: izin yang DITUJUKAN ke guru ini (`p.wali_kelas_id`).
///
/// `wali_id` Some = hanya izin milik guru ini; None = semua (oversight).
/// Syaratnya HARUS sama persis dengan `decide_guru_permit` — kalau tidak,
/// layar menawarkan tombol yang mustahil berhasil (pernah terjadi, lihat
/// catatan di fungsi itu).
pub async fn pending_guru_permits(
    pool: &Pool,
    wali_id: Option<i64>,
    limit: i64,
) -> Result<Vec<PermitQueueRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!(
                "SELECT p.id, u.full_name, u.nis, COALESCE(tc.name, cl.name), \
                p.type, p.start_time, p.end_time, p.reason, p.created_at \
             FROM permit_requests p JOIN users u ON u.id = p.user_id \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             {kelas} \
             WHERE p.guru_status = 'pending' \
                AND ($2::bigint IS NULL \
                     OR COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id) = $2) \
             ORDER BY p.created_at ASC LIMIT $1",
                kelas = super::kelas_utama_lateral("p.user_id"),
            ),
            &[&limit, &wali_id],
        )
        .await
        .context("pending_guru_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PermitQueueRow {
            id: r.get(0),
            student_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kind: r.get(4),
            mulai: r.get(5),
            selesai: r.get(6),
            reason: r.get(7),
            created_at: r.get(8),
        })
        .collect())
}

/// Jumlah izin diputuskan wali kelas (final) HARI INI. `wali_id` Some = hanya
/// keputusan atas izin kelas guru ini.
pub async fn guru_permits_decided_today(pool: &Pool, wali_id: Option<i64>) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            &format!(
                "SELECT COUNT(*) FROM permit_requests p \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             {kelas} \
             WHERE p.guru_status <> 'pending' \
                AND {hari} \
                AND ($1::bigint IS NULL \
                     OR COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id) = $1)",
                kelas = super::kelas_utama_lateral("p.user_id"),
                hari = super::hari_ini_wib("p.guru_at"),
            ),
            &[&wali_id],
        )
        .await
        .context("guru_permits_decided_today")?;
    Ok(row.get(0))
}

/// Keputusan FINAL wali kelas atas SATU baris izin (satu kelas tujuan).
///
/// Guard: `guru_status` masih pending dan (wali_id None ATAU wali kelas tujuan
/// = wali_id). Izin lama tanpa `class_id` jatuh ke kelas utama santri.
///
/// SYARATNYA IDENTIK dengan `pending_guru_permits`. Pernah tidak: query ini dulu
/// menuntut persetujuan tahap kedua sementara antreannya tidak, jadi wali
/// kelas melihat izin di layarnya, menekan Setujui, lalu ditolak dengan pesan
/// yang bahkan tak menyebut sebab sesungguhnya. Daftar dan aksi untuk hal yang
/// sama wajib memakai syarat yang sama.
pub async fn decide_guru_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    wali_id: Option<i64>,
    staff_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p SET guru_status = $2, guru_by = $3, guru_at = NOW() \
             WHERE p.id = $1 AND p.guru_status = 'pending' \
                AND ($4::bigint IS NULL OR COALESCE( \
                     p.wali_kelas_id, \
                     (SELECT c.wali_kelas_id FROM classes c WHERE c.id = p.class_id), \
                     (SELECT c.wali_kelas_id FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id \
                          ORDER BY (c.category = 'kbm') DESC, c.id \
                          LIMIT 1)) = $4)",
            &[&permit_id, &status, &staff_id, &wali_id],
        )
        .await
        .context("decide_guru_permit")?;
    Ok(n > 0)
}

/// Satu santri yang izin/sakitnya SEDANG BERLAKU pada suatu tanggal.
pub struct IzinAktifRow {
    pub permit_id: i64,
    pub user_id: i64,
    pub student_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub kind: String,
    pub mulai: NaiveDateTime,
    pub selesai: NaiveDateTime,
    pub reason: String,
}

/// Santri yang sedang izin/sakit pada `hari`. `user_id` Some = hanya santri itu
/// (dipakai spanduk di layar santri sendiri); None = semua (pantauan staf).
///
/// Yang dihitung "sedang berlaku" adalah izin yang SUDAH DISETUJUI final dan
/// rentangnya mencakup hari itu. Pengajuan yang masih menunggu keputusan
/// SENGAJA tidak ikut: selama belum diputus, santrinya belum berizin — dan
/// daftar yang mencampur keduanya membuat pengurus mengira seseorang sudah
/// dibolehkan padahal belum.
///
/// Izin sehari penuh tersimpan 00:00 → 23:59:59, jadi tak perlu diperlakukan
/// khusus di sini: irisan rentangnya sudah menjawab dengan sendirinya.
pub async fn izin_aktif(
    pool: &Pool,
    hari: NaiveDate,
    user_id: Option<i64>,
    limit: i64,
) -> Result<Vec<IzinAktifRow>> {
    let c = pool.get().await?;
    let sql = format!(
        // "Sedang berlaku pada `hari`" = rentang izinnya menyentuh hari itu:
        // mulai sebelum hari berikutnya, dan selesai setelah awal hari itu.
        "SELECT p.id, p.user_id, u.full_name, u.nis, COALESCE(tc.name, cl.name), \
                p.type, p.start_time, p.end_time, p.reason \
           FROM permit_requests p \
           JOIN users u ON u.id = p.user_id \
           LEFT JOIN classes tc ON tc.id = p.class_id \
           {kelas} \
          WHERE p.guru_status = 'approved' \
            AND p.start_time < ($1::date + INTERVAL '1 day') \
            AND p.end_time >= $1::date \
            AND ($3::bigint IS NULL OR p.user_id = $3) \
          ORDER BY p.end_time, u.full_name LIMIT $2",
        kelas = super::kelas_utama_lateral("p.user_id"),
    );
    let rows = c
        .query(&sql, &[&hari, &limit, &user_id])
        .await
        .context("izin_aktif")?;
    Ok(rows
        .into_iter()
        .map(|r| IzinAktifRow {
            permit_id: r.get(0),
            user_id: r.get(1),
            student_name: r.get(2),
            nis: r.get(3),
            class_name: r.get(4),
            kind: r.get(5),
            mulai: r.get(6),
            selesai: r.get(7),
            reason: r.get(8),
        })
        .collect())
}

/// Tuliskan baris absensi untuk izin yang SUDAH disetujui final.
///
/// MASALAH yang diselesaikan: sampai sekarang tak ada satu pun kode yang
/// menulis `attendances.status = 'permit'/'sick'`. Akibatnya kolom "Izin" di
/// rekap mingguan selalu 0, dan aturan PRD "izin mengurangi poin"
/// (`izin_points` migrasi 28, `attendance_delta("permit")`) tak pernah
/// berjalan — santri berizin sekadar TIDAK PUNYA baris, hanya dilewati
/// auto-absent.
///
/// Status yang ditulis mengikuti JENIS izin:
///   • `sick`  → status 'sick'  → 0 poin (PRD: sakit dgn surat sah tak memotong)
///   • `leave` → status 'permit' → 0 poin (PRD: CUTI juga tak memotong)
///   • lainnya → status 'permit' → −`class_schedules.izin_points` (migrasi 28),
///     HANYA bila kolom itu diisi dan lebih dari 0. NULL atau 0 = kegiatan itu
///     tak memotong poin, dan tak ada baris `point_logs` yang ditulis sama
///     sekali (bukan baris berdelta 0 yang cuma meramaikan buku besar).
///
/// Aturan yang SAMA kini dibaca `attendance::DELTA_SQL`. Dulu jalur itu memberi
/// 0 mati sementara jalur ini memotong sesuai preset, sehingga dua santri
/// dengan izin serupa diperlakukan berbeda semata karena barisnya lahir dari
/// jalur yang berbeda — dan tak satu pun layar memperlihatkan perbedaan itu.
///
/// `leave` = cuti resmi: magang, tugas akhir, lomba mewakili pondok, atau sakit
/// yang butuh perawatan intensif di luar. PRD menyebut sakit dan cuti dalam
/// satu tarikan napas sebagai yang TIDAK mengurangi poin, tapi dulu hanya
/// `sick` yang dibebaskan — cuti jatuh ke cabang "lainnya" dan dipotong persis
/// seperti izin keperluan biasa. Santri yang mewakili pondok berlomba pulang
/// membawa minus poin.
///
/// Statusnya tetap 'permit' (bukan dipaksa jadi 'sick'): rekap membedakan sakit
/// dari izin, dan menandai cuti sebagai sakit akan memalsukan angka itu. Yang
/// dibedakan hanya potongan poinnya.
///
/// `ON CONFLICT DO NOTHING`: baris yang SUDAH ada tak ditimpa. Santri yang
/// ternyata hadir sebagian, atau yang sudah terlanjur dialpakan auto-absent,
/// dibiarkan apa adanya — mengubahnya urusan koreksi manual oleh guru
/// bertugas (migrasi 51), bukan efek samping diam-diam dari persetujuan izin.
///
/// Verifikasi langsung 'approved': yang menyetujui izin adalah wali kelas, dan
/// dialah juga penyetuju akhir absensi. Melewatkannya ke antrean berarti
/// memintanya menyetujui hal yang sama dua kali.
///
/// Return jumlah baris absensi baru.
/// Cakupannya DIHITUNG LANGSUNG dari irisan waktu, bukan dari daftar kelas yang
/// pernah disimpan (`permit_request_classes` dihapus migrasi 86): sebuah sesi
/// terlewat bila jam sesinya beririsan dengan rentang izin dan santrinya
/// peserta kelas itu. Satu aturan, satu jawaban — dan tak ada tabel perantara
/// yang bisa basi ketika jadwal berubah setelah izin disetujui.
pub async fn materialize_permit_attendance(pool: &Pool, permit_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "WITH p AS ( \
                SELECT pr.id, pr.user_id, pr.type, pr.start_time, pr.end_time, \
                       CASE WHEN pr.type = 'sick' THEN 'sick' ELSE 'permit' END AS att_status \
                  FROM permit_requests pr \
                 WHERE pr.id = $1 AND pr.guru_status = 'approved' \
             ), \
             ins AS ( \
                INSERT INTO attendances \
                    (user_id, class_session_id, class_schedule_id, status, method, \
                     verify_status, verified_at, \
                     note, gate_label, scanned_at, scan_date) \
                SELECT p.user_id, s.id, s.class_schedule_id, p.att_status, 'manual', \
                       'approved', NOW(), \
                       'Izin disetujui', 'system', NOW(), s.session_date \
                  FROM p \
                  JOIN class_participants cp ON cp.user_id = p.user_id \
                  JOIN class_sessions s ON s.class_id = cp.class_id \
                   AND s.status <> 'cancelled' \
                  JOIN class_schedules sj ON sj.id = s.class_schedule_id \
                 WHERE (s.session_date + sj.start_time) < p.end_time \
                   AND (s.session_date + sj.end_time) > p.start_time \
                 ON CONFLICT (user_id, class_session_id) DO NOTHING \
                RETURNING id, user_id, class_schedule_id, status \
             ), \
             lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category, attendance_id) \
                SELECT ins.user_id, -sch.izin_points::int, \
                       'Kehadiran (' || ins.status || ') — izin disetujui', 'discipline', ins.id \
                  FROM ins \
                  JOIN class_schedules sch ON sch.id = ins.class_schedule_id \
                  CROSS JOIN p \
                 WHERE ins.status = 'permit' \
                   AND p.type NOT IN ('sick', 'leave') \
                   AND COALESCE(sch.izin_points, 0) > 0 \
                RETURNING user_id \
             ) \
             SELECT COUNT(*)::bigint FROM ins",
            &[&permit_id],
        )
        .await
        .context("materialize_permit_attendance")?;
    Ok(row.get(0))
}
