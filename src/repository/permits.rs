//! repository/permits.rs — Query tabel permit_requests (izin/sakit/keperluan).
//!
//! Migrasi 46 — izin PER-KELAS. Satu pengajuan izin memecah diri jadi beberapa
//! baris `permit_requests`: satu untuk tiap WALI KELAS yang kelasnya dilewati
//! selama rentang izin. Contoh: izin 2 hari melewati kelas A (wali X), B & C
//! (wali Y) → 2 baris, satu ke wali X satu ke wali Y.
//!
//! Alur per-baris: Pamong kelas (bila `require_pamong` DAN kelas itu punya
//! pamong) → Wali Kelas (FINAL). Kelas ber-`require_pamong` tapi `pamong_id`
//! masih NULL TIDAK memblokir izin — tahap pamong dilewati, izin langsung ke
//! wali kelas. Tanpa ini izin macet permanen: tak ada pamong yang cocok untuk
//! menyetujui, sementara tahap final menuntut `pamong_status = 'approved'`.
//! Orang tua TIDAK lagi jadi penyetuju (kolom parent_* dihapus di migrasi 46) —
//! izin adalah urusan akademik; orang tua cukup dinotifikasi.

use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveTime};
use deadpool_postgres::Pool;

/// Target notifikasi izin: penyetuju kelas UTAMA santri (wali kelas selalu;
/// pamong hanya bila `require_pamong` = verifikasi 2 langkah).
pub struct PermitNotifyTargets {
    pub student_name: String,
    pub require_pamong: bool,
    pub wali_name: Option<String>,
    pub wali_phone: Option<String>,
    pub pamong_name: Option<String>,
    pub pamong_phone: Option<String>,
}

/// Ambil wali kelas + pamong (nama & HP) penyetuju izin.
///
/// `class_id` Some = kelas TUJUAN izin (migrasi 46) — dipakai saat satu ajuan
/// terpecah ke beberapa wali; None = fallback ke kelas KBM santri (migrasi 65),
/// untuk izin lama atau santri tanpa kelas terjadwal. Kelas KBM-lah yang punya
/// wali kelas penanggung jawab; kelas non-KBM (piket, apel) kerap tak punya.
pub async fn permit_notify_targets(
    pool: &Pool,
    student_id: i64,
    class_id: Option<i64>,
) -> Result<Option<PermitNotifyTargets>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT s.full_name, \
                    COALESCE(cl.require_pamong, TRUE), \
                    w.full_name, w.phone_number, \
                    pm.full_name, pm.phone_number \
             FROM users s \
             LEFT JOIN LATERAL ( \
                 SELECT cp_ku.class_id FROM class_participants cp_ku \
                   JOIN classes c ON c.id = cp_ku.class_id \
                  WHERE cp_ku.user_id = s.id AND $2::bigint IS NULL \
                  ORDER BY (c.category = 'kbm') DESC, c.id \
                  LIMIT 1 \
             ) cp ON TRUE \
             LEFT JOIN classes cl ON cl.id = COALESCE($2::bigint, cp.class_id) \
             LEFT JOIN users w  ON w.id = cl.wali_kelas_id \
             LEFT JOIN users pm ON pm.id = cl.pamong_id \
             WHERE s.id = $1 \
             LIMIT 1",
            &[&student_id, &class_id],
        )
        .await
        .context("permit_notify_targets")?;
    Ok(row.map(|r| PermitNotifyTargets {
        student_name: r.get(0),
        require_pamong: r.get(1),
        wali_name: r.get(2),
        wali_phone: r.get(3),
        pamong_name: r.get(4),
        pamong_phone: r.get(5),
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
    pub require_pamong: bool,
    /// once|daily|weekly|monthly|custom — ditafsirkan `dates_in_range`.
    pub recurrence_type: String,
    pub sched_start: NaiveDate,
    pub sched_end: Option<NaiveDate>,
    /// Tanggal manual untuk recurrence 'custom' ("YYYY-MM-DD"). Kosong untuk
    /// pola lain. WAJIB ikut: seluruh jadwal KBM di produksi memakai custom,
    /// dan tanpa daftar ini pola-nya tak bisa diuji sama sekali.
    pub custom_dates: Vec<String>,
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
    jam: Option<(NaiveTime, NaiveTime)>,
) -> Result<Vec<AffectedClass>> {
    let (jam_mulai, jam_selesai) = match jam {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    };
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
            // Izin PER JAM (migrasi 66) menyaring lagi lewat jam jadwal: dua
            // rentang bersinggungan bila `mulai_a < selesai_b DAN selesai_a >
            // mulai_b`. Izin 09:00–11:00 karena itu tak menyentuh kelas subuh
            // maupun apel malam di hari yang sama.
            "SELECT cl.id, cl.name, cl.wali_kelas_id, w.full_name, \
                    COALESCE(cl.require_pamong, TRUE), \
                    cs.recurrence_type, cs.start_date, cs.end_date, \
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
               AND ($4::time IS NULL OR (cs.start_time < $5 AND cs.end_time > $4)) \
             ORDER BY cl.wali_kelas_id NULLS LAST, cl.name, cs.id",
            &[&student_id, &start_date, &end_date, &jam_mulai, &jam_selesai],
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
            require_pamong: r.get(4),
            recurrence_type: r.get(5),
            sched_start: r.get(6),
            sched_end: r.get(7),
            custom_dates: r
                .get::<_, Option<serde_json::Value>>(8)
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
            "SELECT p.id, cl.name, COUNT(s.id)::bigint \
             FROM permit_requests p \
             JOIN permit_request_classes prc ON prc.permit_id = p.id \
             JOIN classes cl ON cl.id = prc.class_id \
             LEFT JOIN class_sessions s ON s.class_id = prc.class_id \
                  AND s.status <> 'cancelled' \
                  AND s.session_date BETWEEN p.start_date \
                                         AND COALESCE(p.end_date, p.start_date) \
             LEFT JOIN class_schedules sch ON sch.id = s.class_schedule_id \
             WHERE p.id = ANY($1::bigint[]) \
               AND (p.start_time IS NULL \
                    OR (sch.start_time < p.end_time AND sch.end_time > p.start_time)) \
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

/// Catat kelas-kelas yang DICAKUP sebuah izin (migrasi 64).
///
/// Terpisah dari `permit_requests.class_id` yang cuma kelas acuan persetujuan:
/// izin satu wali kerap mencakup beberapa kelas sekaligus, dan sebelum tabel
/// ini ada, kelas selain yang pertama tak tercatat di mana pun — sesinya tetap
/// di-alpa-kan otomatis meski izinnya sudah disetujui.
pub async fn insert_permit_classes(pool: &Pool, permit_id: i64, class_ids: &[i64]) -> Result<u64> {
    if class_ids.is_empty() {
        return Ok(0);
    }
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO permit_request_classes (permit_id, class_id) \
             SELECT $1, cid FROM unnest($2::bigint[]) AS cid \
             ON CONFLICT DO NOTHING",
            &[&permit_id, &class_ids],
        )
        .await
        .context("insert_permit_classes")?;
    Ok(n)
}

/// Sisipkan SATU baris izin yang ditujukan ke satu kelas + wali kelas tertentu.
/// Dipanggil berulang oleh `auto_create_permits_per_wali` (sekali per wali unik).
#[allow(clippy::too_many_arguments)]
pub async fn insert_permit(
    pool: &Pool,
    user_id: i64,
    requested_by: i64,
    kind: &str,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    // `jam`: berlaku sama untuk setiap hari dalam rentang; None = sehari
    // penuh (migrasi 66).
    jam: Option<(NaiveTime, NaiveTime)>,
    reason: &str,
    class_id: Option<i64>,
    wali_kelas_id: Option<i64>,
) -> Result<i64> {
    let (jam_mulai, jam_selesai) = match jam {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    };
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO permit_requests \
                (user_id, requested_by, type, reason, start_date, end_date, class_id, \
                 wali_kelas_id, start_time, end_time) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            &[
                &user_id,
                &requested_by,
                &kind,
                &reason,
                &start_date,
                &end_date,
                &class_id,
                &wali_kelas_id,
                &jam_mulai,
                &jam_selesai,
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
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub pamong_status: String,
    pub guru_status: String,
    /// Rute lewat pamong? Diturunkan dari KELAS tujuan izin (migrasi 46),
    /// fallback ke kelas utama santri untuk baris lama hasil backfill.
    pub require_pamong: bool,
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
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub pamong_status: String,
    pub guru_status: String,
    pub require_pamong: bool,
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
                p.start_date, p.end_date, p.start_time, p.end_time, \
                p.pamong_status, p.guru_status, \
                COALESCE(tc.require_pamong, cl.require_pamong, TRUE), \
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
        start_date: r.get(5),
        end_date: r.get(6),
        start_time: r.get(7),
        end_time: r.get(8),
        pamong_status: r.get(9),
        guru_status: r.get(10),
        require_pamong: r.get(11),
        class_name: r.get(12),
        wali_kelas_id: r.get(13),
        wali_name: r.get(14),
        requested_by: r.get(15),
        requester_name: r.get::<_, Option<String>>(16).unwrap_or_default(),
        requester_role: r.get::<_, Option<String>>(17).unwrap_or_default(),
        created_at: r.get(18),
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
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    jam: Option<(NaiveTime, NaiveTime)>,
    reason: &str,
) -> Result<bool> {
    let (jam_mulai, jam_selesai) = match jam {
        Some((a, b)) => (Some(a), Some(b)),
        None => (None, None),
    };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p \
                SET type = $3, start_date = $4, end_date = $5, \
                    start_time = $6, end_time = $7, reason = $8 \
              WHERE p.id = $1 \
                AND p.guru_status = 'pending' \
                AND (p.user_id = $2 OR p.wali_kelas_id = $2)",
            &[
                &permit_id,
                &actor_id,
                &kind,
                &start_date,
                &end_date,
                &jam_mulai,
                &jam_selesai,
                &reason,
            ],
        )
        .await
        .context("update_permit")?;
    Ok(n > 0)
}

/// Ganti seluruh cakupan kelas sebuah izin (dipakai setelah tanggalnya diubah).
///
/// Hapus-lalu-sisipkan, bukan menambahkan: mengubah rentang izin bisa membuat
/// kelas yang tadinya terdampak jadi tak terdampak lagi, dan baris lama yang
/// tertinggal akan terus membebaskan santri dari auto-alpa di kelas yang
/// sebenarnya sudah di luar izinnya.
pub async fn ganti_cakupan_izin(pool: &Pool, permit_id: i64, class_ids: &[i64]) -> Result<()> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("ganti_cakupan tx")?;
    tx.execute(
        "DELETE FROM permit_request_classes WHERE permit_id = $1",
        &[&permit_id],
    )
    .await
    .context("ganti_cakupan hapus")?;
    if !class_ids.is_empty() {
        tx.execute(
            "INSERT INTO permit_request_classes (permit_id, class_id) \
             SELECT $1, cid FROM unnest($2::bigint[]) AS cid ON CONFLICT DO NOTHING",
            &[&permit_id, &class_ids],
        )
        .await
        .context("ganti_cakupan sisip")?;
    }
    tx.commit().await.context("ganti_cakupan commit")?;
    Ok(())
}

pub async fn list_my_permits(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<PermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!(
                "SELECT p.type, p.start_date, p.end_date, p.pamong_status, p.guru_status, \
                    COALESCE(tc.require_pamong, cl.require_pamong, TRUE), tc.name, \
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
            start_date: r.get(1),
            end_date: r.get(2),
            pamong_status: r.get(3),
            guru_status: r.get(4),
            require_pamong: r.get(5),
            class_name: r.get(6),
            id: r.get(7),
            oleh_ortu: r.get(8),
            requester_name: r.get(9),
        })
        .collect())
}

// CATATAN (migrasi 46): tahap konfirmasi ORANG TUA DIHAPUS. Fungsi
// `pending_parent_confirms` & `confirm_parent_permit` ikut dihapus — orang tua
// kini hanya MELIHAT izin anaknya (lihat repository/parents.rs), tak memutus.

// ── Tahap 1: PAMONG kelas ─────────────────────────────────────────────────────

pub struct PendingPamongRow {
    pub id: i64,
    pub student_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Jam berlaku bila izinnya per jam (migrasi 66); None = sehari penuh.
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    /// Kelas tujuan memakai verifikasi dua langkah DAN punya pamong — penentu
    /// apakah indikator kemajuan menampilkan tahap pamong.
    pub dua_tahap: bool,
    /// Tahap pamong sudah disetujui? (Tidak memblokir wali — lihat
    /// `pending_guru_permits`.)
    pub pamong_ok: bool,
}

/// Antrean pamong: izin yang menuju KELAS yang wajib via pamong
/// (`require_pamong`), menunggu keputusan pamong kelas itu. Guard `pamong_id`
/// memakai pamong KELAS TUJUAN izin (`p.class_id`), bukan kelas utama santri —
/// inilah inti migrasi 46: izin ke kelas X hanya boleh diputus pamong kelas X.
/// `default_require` = fallback bila izin lama belum punya `class_id`.
pub async fn pending_pamong_permits(
    pool: &Pool,
    default_require: bool,
    pamong_id: Option<i64>,
    limit: i64,
) -> Result<Vec<PendingPamongRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!(
                "SELECT p.id, u.full_name, u.nis, COALESCE(tc.name, cl.name), \
                p.type, p.start_date, p.end_date, p.reason, p.created_at, \
                p.start_time, p.end_time, \
                (COALESCE(tc.require_pamong, cl.require_pamong, $2) \
                 AND COALESCE(tc.pamong_id, cl.pamong_id) IS NOT NULL) AS dua_tahap, \
                (p.pamong_status = 'approved') AS pamong_ok \
             FROM permit_requests p JOIN users u ON u.id = p.user_id \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             {kelas} \
             WHERE p.pamong_status = 'pending' AND p.guru_status = 'pending' \
                AND COALESCE(tc.require_pamong, cl.require_pamong, $2) = TRUE \
                AND ($3::bigint IS NULL OR COALESCE(tc.pamong_id, cl.pamong_id) = $3) \
             ORDER BY p.created_at ASC LIMIT $1",
                kelas = super::kelas_utama_lateral("p.user_id"),
            ),
            &[&limit, &default_require, &pamong_id],
        )
        .await
        .context("pending_pamong_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingPamongRow {
            id: r.get(0),
            student_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kind: r.get(4),
            start_date: r.get(5),
            end_date: r.get(6),
            reason: r.get(7),
            created_at: r.get(8),
            start_time: r.get(9),
            end_time: r.get(10),
            dua_tahap: r.get(11),
            pamong_ok: r.get(12),
        })
        .collect())
}

/// Jumlah izin diputuskan pamong HARI INI (statistik antrean).
pub async fn pamong_permits_decided_today(pool: &Pool, pamong_id: Option<i64>) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            &format!(
                "SELECT COUNT(*) FROM permit_requests p \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             {kelas} \
             WHERE p.pamong_status <> 'pending' \
                AND {hari} \
                AND ($1::bigint IS NULL OR COALESCE(tc.pamong_id, cl.pamong_id) = $1)",
                kelas = super::kelas_utama_lateral("p.user_id"),
                hari = super::hari_ini_wib("p.pamong_at"),
            ),
            &[&pamong_id],
        )
        .await
        .context("pamong_permits_decided_today")?;
    Ok(row.get(0))
}

/// Setujui/tolak izin oleh pamong (tahap 1). Guard: KELAS TUJUAN izin
/// (`p.class_id`) wajib via pamong (`require_pamong`) DAN pamong kelas itu =
/// `pamong_id`. Izin lama tanpa `class_id` jatuh ke kelas utama santri.
pub async fn decide_pamong_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    default_require: bool,
    pamong_id: Option<i64>,
    staff_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p SET pamong_status = $2, pamong_by = $3, pamong_at = NOW() \
             WHERE p.id = $1 AND p.pamong_status = 'pending' AND p.guru_status = 'pending' \
                AND COALESCE( \
                    (SELECT c.require_pamong FROM classes c WHERE c.id = p.class_id), \
                    (SELECT c.require_pamong FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id \
                          ORDER BY (c.category = 'kbm') DESC, c.id \
                          LIMIT 1), $4) = TRUE \
                AND ($5::bigint IS NULL OR COALESCE( \
                    (SELECT c.pamong_id FROM classes c WHERE c.id = p.class_id), \
                    (SELECT c.pamong_id FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id \
                          ORDER BY (c.category = 'kbm') DESC, c.id \
                          LIMIT 1)) = $5)",
            &[&permit_id, &status, &staff_id, &default_require, &pamong_id],
        )
        .await
        .context("decide_pamong_permit")?;
    Ok(n > 0)
}

// ── Tahap FINAL: WALI KELAS (guru penyetuju akhir) ────────────────────────────

/// Antrean wali kelas: izin yang DITUJUKAN ke guru ini (`p.wali_kelas_id`).
///
/// TIDAK menunggu pamong. Dulu izin di kelas dua-langkah baru muncul setelah
/// `pamong_status = 'approved'` — akibatnya izin bisa mengendap tak terlihat
/// oleh satu-satunya orang yang berhak memutuskannya, dan santri menunggu
/// tanpa tahu ke siapa harus bertanya. Tahap pamong tetap ada sebagai catatan
/// (dan ditampilkan di indikator kemajuan), tapi ia menyaring, bukan
/// memblokir. Pamong yang MENOLAK tetap menghentikan izin.
///
/// `wali_id` Some = hanya izin milik guru ini; None = semua (dewan guru/admin
/// oversight). `default_require` kini hanya dipakai pemanggil untuk label.
pub async fn pending_guru_permits(
    pool: &Pool,
    wali_id: Option<i64>,
    default_require: bool,
    limit: i64,
) -> Result<Vec<PendingPamongRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!(
                "SELECT p.id, u.full_name, u.nis, COALESCE(tc.name, cl.name), \
                p.type, p.start_date, p.end_date, p.reason, p.created_at, \
                p.start_time, p.end_time, \
                (COALESCE(tc.require_pamong, cl.require_pamong, $2) \
                 AND COALESCE(tc.pamong_id, cl.pamong_id) IS NOT NULL) AS dua_tahap, \
                (p.pamong_status = 'approved') AS pamong_ok \
             FROM permit_requests p JOIN users u ON u.id = p.user_id \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             {kelas} \
             WHERE p.guru_status = 'pending' \
                AND p.pamong_status <> 'rejected' \
                AND ($3::bigint IS NULL \
                     OR COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id) = $3) \
             ORDER BY p.created_at ASC LIMIT $1",
                kelas = super::kelas_utama_lateral("p.user_id"),
            ),
            &[&limit, &default_require, &wali_id],
        )
        .await
        .context("pending_guru_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingPamongRow {
            id: r.get(0),
            student_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kind: r.get(4),
            start_date: r.get(5),
            end_date: r.get(6),
            reason: r.get(7),
            created_at: r.get(8),
            start_time: r.get(9),
            end_time: r.get(10),
            dua_tahap: r.get(11),
            pamong_ok: r.get(12),
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
/// Guard: bila kelas tujuan `require_pamong` maka pamong wajib sudah approve;
/// `guru_status` masih pending; dan (wali_id None ATAU wali kelas tujuan =
/// wali_id). Izin lama tanpa `class_id` jatuh ke kelas utama santri.
pub async fn decide_guru_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    wali_id: Option<i64>,
    default_require: bool,
    staff_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p SET guru_status = $2, guru_by = $3, guru_at = NOW() \
             WHERE p.id = $1 AND p.guru_status = 'pending' \
                AND p.pamong_status <> 'rejected' \
                AND CASE WHEN COALESCE( \
                        (SELECT c.require_pamong FROM classes c WHERE c.id = p.class_id), \
                        (SELECT c.require_pamong FROM class_participants cp \
                            JOIN classes c ON c.id = cp.class_id \
                            WHERE cp.user_id = p.user_id \
                          ORDER BY (c.category = 'kbm') DESC, c.id \
                          LIMIT 1), $5) \
                          AND COALESCE( \
                        (SELECT c.pamong_id FROM classes c WHERE c.id = p.class_id), \
                        (SELECT c.pamong_id FROM class_participants cp \
                            JOIN classes c ON c.id = cp.class_id \
                            WHERE cp.user_id = p.user_id \
                          ORDER BY (c.category = 'kbm') DESC, c.id \
                          LIMIT 1)) IS NOT NULL \
                     THEN p.pamong_status = 'approved' \
                     ELSE TRUE END \
                AND ($4::bigint IS NULL OR COALESCE( \
                     p.wali_kelas_id, \
                     (SELECT c.wali_kelas_id FROM classes c WHERE c.id = p.class_id), \
                     (SELECT c.wali_kelas_id FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id \
                          ORDER BY (c.category = 'kbm') DESC, c.id \
                          LIMIT 1)) = $4)",
            &[&permit_id, &status, &staff_id, &wali_id, &default_require],
        )
        .await
        .context("decide_guru_permit")?;
    Ok(n > 0)
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
/// dibiarkan apa adanya — mengubahnya urusan koreksi manual oleh guru/pamong
/// bertugas (migrasi 51), bukan efek samping diam-diam dari persetujuan izin.
///
/// Verifikasi langsung 'approved': yang menyetujui izin adalah wali kelas, dan
/// dialah juga penyetuju akhir absensi. Melewatkannya ke antrean berarti
/// memintanya menyetujui hal yang sama dua kali.
///
/// Return jumlah baris absensi baru.
pub async fn materialize_permit_attendance(pool: &Pool, permit_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "WITH p AS ( \
                SELECT pr.id, pr.user_id, pr.class_id, pr.type, \
                       pr.start_date, COALESCE(pr.end_date, pr.start_date) AS end_date, \
                       pr.start_time, pr.end_time, \
                       CASE WHEN pr.type = 'sick' THEN 'sick' ELSE 'permit' END AS att_status \
                  FROM permit_requests pr \
                 WHERE pr.id = $1 AND pr.guru_status = 'approved' \
             ), \
             ins AS ( \
                INSERT INTO attendances \
                    (user_id, class_session_id, class_schedule_id, status, method, \
                     pamong_status, pamong_at, verify_status, verified_at, \
                     note, gate_label, scanned_at, scan_date) \
                SELECT p.user_id, s.id, s.class_schedule_id, p.att_status, 'manual', \
                       'approved', NOW(), 'approved', NOW(), \
                       'Izin disetujui', 'system', NOW(), s.session_date \
                  FROM p \
                  JOIN permit_request_classes prc ON prc.permit_id = p.id \
                  JOIN class_sessions s ON s.class_id = prc.class_id \
                   AND s.session_date BETWEEN p.start_date AND p.end_date \
                   AND s.status <> 'cancelled' \
                  LEFT JOIN class_schedules sj ON sj.id = s.class_schedule_id \
                 WHERE p.start_time IS NULL \
                    OR (sj.start_time < p.end_time AND sj.end_time > p.start_time) \
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
