//! repository/attendance.rs — Query tabel attendances.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use deadpool_postgres::Pool;

pub struct AttRow {
    pub status: String,
    pub gate_label: Option<String>,
    pub scanned_at: DateTime<Utc>,
    pub verify_status: String,
}

pub struct WeeklyRecapRawRow {
    pub name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub hadir: i64,
    pub telat: i64,
    pub izin: i64,
    pub alpa: i64,
    pub points: i32,
}

/// Satu santri dengan net poin mingguan ≤ ambang (pemanggilan PRD hal. 12).
pub struct WeeklyNetRow {
    pub name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub net: i32,
}

/// Santri dengan TOTAL net poin (SUM point_logs.delta) pekan [start,end] WIB
/// ≤ -9 (ambang pemanggilan terendah). Terurut paling minus dulu.
pub async fn weekly_net_points(
    pool: &Pool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<WeeklyNetRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT u.full_name, u.nis, cl.name, SUM(pl.delta)::int AS net \
         FROM point_logs pl \
         JOIN users u ON u.id = pl.user_id AND u.role IN ('santri', 'santri_finance') \
         {kelas} \
         WHERE (pl.created_at AT TIME ZONE 'Asia/Jakarta')::date BETWEEN $1 AND $2 \
         GROUP BY u.id, u.full_name, u.nis, cl.name \
         HAVING SUM(pl.delta) <= -9 \
         ORDER BY net ASC",
        kelas = super::kelas_utama_lateral("u.id"),
    );
    let rows = c
        .query(&sql, &[&start, &end])
        .await
        .context("weekly_net_points")?;
    Ok(rows
        .into_iter()
        .map(|r| WeeklyNetRow {
            name: r.get(0),
            nis: r.get(1),
            class_name: r.get(2),
            net: r.get(3),
        })
        .collect())
}

/// Baris hitung kehadiran per (santri, jenis kegiatan) untuk satu pekan —
/// dasar perhitungan reward mingguan (PRD).
pub struct WeeklyCatCount {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    /// activity_type efektif: kbm|non_kbm|piket|other.
    pub activity_type: String,
    pub hadir: i64,
    pub telat: i64,
    pub izin: i64,
    pub sakit: i64,
    pub alfa: i64,
}

/// Hitung kehadiran per santri PER KATEGORI kegiatan (dari activity_type jadwal)
/// dalam rentang [start,end] WIB. Hanya santri yang punya catatan (INNER JOIN).
/// telat menyertakan 'outside_schedule'.
pub async fn weekly_counts_by_category(
    pool: &Pool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<WeeklyCatCount>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, COALESCE(sch.activity_type, 'other'), \
                COUNT(*) FILTER (WHERE a.status = 'present'), \
                COUNT(*) FILTER (WHERE a.status IN ('late','outside_schedule')), \
                COUNT(*) FILTER (WHERE a.status = 'permit'), \
                COUNT(*) FILTER (WHERE a.status = 'sick'), \
                COUNT(*) FILTER (WHERE a.status = 'absent') \
             FROM users u \
             JOIN attendances a ON a.user_id = u.id \
                AND a.scan_date BETWEEN $1 AND $2 \
             LEFT JOIN class_schedules sch ON sch.id = a.class_schedule_id \
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
             GROUP BY u.id, u.full_name, u.nis, COALESCE(sch.activity_type, 'other') \
             ORDER BY u.full_name",
            &[&start, &end],
        )
        .await
        .context("weekly_counts_by_category")?;
    Ok(rows
        .into_iter()
        .map(|r| WeeklyCatCount {
            user_id: r.get(0),
            name: r.get(1),
            nis: r.get(2),
            activity_type: r.get(3),
            hadir: r.get(4),
            telat: r.get(5),
            izin: r.get(6),
            sakit: r.get(7),
            alfa: r.get(8),
        })
        .collect())
}

/// Kreditkan reward mingguan satu santri (idempotent: UNIQUE user_id,week_start).
/// Return true bila BARU dikreditkan; false bila sudah pernah (skip). Menulis
/// weekly_rewards + point_logs + menaikkan users.points dalam satu transaksi.
pub async fn credit_weekly_reward(
    pool: &Pool,
    user_id: i64,
    week_start: NaiveDate,
    points: i32,
    detail: &str,
) -> Result<bool> {
    if points <= 0 {
        return Ok(false);
    }
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("credit_weekly_reward tx")?;
    let ins = tx
        .query_opt(
            "INSERT INTO weekly_rewards (user_id, week_start, points, detail) \
             VALUES ($1, $2, $3, $4) ON CONFLICT (user_id, week_start) DO NOTHING \
             RETURNING id",
            &[&user_id, &week_start, &points, &detail],
        )
        .await
        .context("credit_weekly_reward insert")?;
    if ins.is_none() {
        tx.rollback().await.ok();
        return Ok(false);
    }
    let reason = format!("Reward mingguan {week_start}");
    // users.points diperbarui OTOMATIS oleh trigger trg_point_logs_balance
    // (migrasi 32) — cukup tulis point_logs.
    tx.execute(
        "INSERT INTO point_logs (user_id, delta, reason, category) \
         VALUES ($1, $2, $3, 'achievement')",
        &[&user_id, &points, &reason],
    )
    .await
    .context("credit_weekly_reward point_logs")?;
    tx.commit().await.context("credit_weekly_reward commit")?;
    Ok(true)
}

/// Set user_id yang SUDAH menerima reward pekan `week_start` (agar UI tahu).
pub async fn credited_users_for_week(pool: &Pool, week_start: NaiveDate) -> Result<Vec<i64>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT user_id FROM weekly_rewards WHERE week_start = $1",
            &[&week_start],
        )
        .await
        .context("credited_users_for_week")?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}

/// Rekap kehadiran per-santri untuk rentang tanggal (WIB). Semua santri aktif
/// dimasukkan (LEFT JOIN) walau tanpa catatan pekan itu (semua nol).
/// telat menyertakan 'outside_schedule' (hadir di luar jadwal).
pub async fn weekly_recap(
    pool: &Pool,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<Vec<WeeklyRecapRawRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, u.nis, \
                (SELECT c.name FROM class_participants cp JOIN classes c ON c.id = cp.class_id \
                    WHERE cp.user_id = u.id \
                    ORDER BY (c.category = 'kbm') DESC, c.id \
                    LIMIT 1), \
                COUNT(*) FILTER (WHERE a.status = 'present'), \
                COUNT(*) FILTER (WHERE a.status IN ('late','outside_schedule')), \
                COUNT(*) FILTER (WHERE a.status IN ('permit','sick')), \
                COUNT(*) FILTER (WHERE a.status = 'absent'), \
                u.points \
             FROM users u \
             LEFT JOIN attendances a ON a.user_id = u.id \
                AND a.scan_date BETWEEN $1 AND $2 \
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
             GROUP BY u.id, u.full_name, u.nis, u.points \
             ORDER BY u.full_name",
            &[&start, &end],
        )
        .await
        .context("weekly_recap")?;
    Ok(rows
        .into_iter()
        .map(|r| WeeklyRecapRawRow {
            name: r.get(0),
            nis: r.get(1),
            class_name: r.get(2),
            hadir: r.get(3),
            telat: r.get(4),
            izin: r.get(5),
            alpa: r.get(6),
            points: r.get(7),
        })
        .collect())
}

/// Riwayat kehadiran terakhir milik satu santri.
pub async fn recent_attendances(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<AttRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT status, gate_label, scanned_at, verify_status \
             FROM attendances WHERE user_id = $1 \
             ORDER BY scanned_at DESC LIMIT $2",
            &[&user_id, &limit],
        )
        .await
        .context("recent_attendances")?;
    Ok(rows
        .into_iter()
        .map(|r| AttRow {
            status: r.get(0),
            gate_label: r.get(1),
            scanned_at: r.get(2),
            verify_status: r.get(3),
        })
        .collect())
}

/// Progress bulan ini: (hadir_termasuk_terlambat, total_catatan).
pub async fn month_progress(pool: &Pool, user_id: i64) -> Result<(i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FILTER (WHERE status IN ('present','late')), COUNT(*) \
             FROM attendances \
             WHERE user_id = $1 AND scan_date >= date_trunc('month', NOW() AT TIME ZONE 'Asia/Jakarta')::date",
            &[&user_id],
        )
        .await?;
    Ok((row.get(0), row.get(1)))
}

pub struct PendingRow {
    pub id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub scanned_at: DateTime<Utc>,
    pub gate_label: Option<String>,
    pub status: String,
}

/// Antrean verifikasi pamong (pamong_status = pending), tertua dulu. `pamong_id`
/// Some = hanya kehadiran santri di KELAS yang pamongnya guru ini (migrasi 30);
/// None = semua (admin/dewan oversight).
pub async fn pending_pamong(
    pool: &Pool,
    pamong_id: Option<i64>,
    limit: i64,
) -> Result<Vec<PendingRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT a.id, u.full_name, u.nis, cl.name, a.scanned_at, a.gate_label, a.status \
             FROM attendances a \
             JOIN users u ON u.id = a.user_id \
             LEFT JOIN class_sessions cs ON cs.id = a.class_session_id \
             LEFT JOIN classes cl ON cl.id = cs.class_id \
             WHERE a.pamong_status = 'pending' \
                AND COALESCE(cl.require_pamong, TRUE) = TRUE \
                AND COALESCE(cs.pamong_id, cl.pamong_id) IS NOT NULL \
                AND ($2::bigint IS NULL OR COALESCE(cs.pamong_id, cl.pamong_id) = $2) \
             ORDER BY a.scanned_at ASC LIMIT $1",
            &[&limit, &pamong_id],
        )
        .await
        .context("pending_pamong")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            scanned_at: r.get(4),
            gate_label: r.get(5),
            status: r.get(6),
        })
        .collect())
}

/// Jumlah yang sudah disetujui pamong hari ini.
pub async fn approved_today(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            &format!(
                "SELECT COUNT(*) FROM attendances \
                 WHERE pamong_status = 'approved' AND {hari}",
                hari = super::hari_ini_wib("pamong_at"),
            ),
            &[],
        )
        .await?;
    Ok(row.get(0))
}

/// Setujui/tolak (tahap pamong) DALAM SATU TRANSAKSI. Saat disetujui, poin
/// kehadiran diberikan (models::attendance::point_rule): insert point_logs +
/// update saldo users.points. Return true bila ada baris ter-update.
/// Daftar santri satu SESI yang menunggu verifikasi pada `stage` ("pamong" |
/// "final"), dibatasi kepemilikan bila `actor_id` Some (pamong/wali sesi itu);
/// None = admin/dewan (semua). Dipakai panel verifikasi per-sesi di detail sesi.
/// `actor_pamong = true` → `actor_id` dicocokkan ke PAMONG sesi, bukan guru.
///
/// Penanda ini bukan hiasan. Tahap "final" biasanya dijalankan guru, jadi dulu
/// `actor_id` selalu diadu dengan `COALESCE(cs.teacher_id, cl.wali_kelas_id)`.
/// Sejak mode kelas `verify_mode='pamong'` ada, tahap final bisa dijalankan
/// PAMONG — mengadu id pamong dengan kolom guru tak akan pernah cocok, dan
/// membiarkan `actor_id` NULL (perilaku lama) berarti pamong mana pun boleh
/// memfinalkan sesi kelas siapa pun.
pub async fn session_verify_list(
    pool: &Pool,
    session_id: i64,
    stage: &str,
    actor_id: Option<i64>,
    actor_pamong: bool,
) -> Result<Vec<PendingRow>> {
    let c = pool.get().await?;
    let sql = if stage == "pamong" {
        "SELECT a.id, u.full_name, u.nis, cl.name, a.scanned_at, a.gate_label, a.status \
         FROM attendances a JOIN users u ON u.id = a.user_id \
         LEFT JOIN class_sessions cs ON cs.id = a.class_session_id \
         LEFT JOIN classes cl ON cl.id = cs.class_id \
         WHERE a.class_session_id = $1 AND a.pamong_status = 'pending' \
           AND COALESCE(cl.require_pamong, TRUE) = TRUE \
           AND COALESCE(cs.pamong_id, cl.pamong_id) IS NOT NULL \
           AND ($2::bigint IS NULL OR COALESCE(cs.pamong_id, cl.pamong_id) = $2) \
         ORDER BY u.full_name"
    } else {
        "SELECT a.id, u.full_name, u.nis, cl.name, a.scanned_at, a.gate_label, a.status \
         FROM attendances a JOIN users u ON u.id = a.user_id \
         LEFT JOIN class_sessions cs ON cs.id = a.class_session_id \
         LEFT JOIN classes cl ON cl.id = cs.class_id \
         WHERE a.class_session_id = $1 AND a.verify_status = 'pending' \
           AND a.pamong_status <> 'rejected' \
           AND ($2::bigint IS NULL OR \
                CASE WHEN $3::bool THEN COALESCE(cs.pamong_id, cl.pamong_id) \
                     ELSE COALESCE(cs.teacher_id, cl.wali_kelas_id) END = $2) \
         ORDER BY u.full_name"
    };
    // Cabang "pamong" tak memakai $3 (tahapnya memang selalu milik pamong),
    // jadi parameternya pun tak dikirim — Postgres menolak bind yang jumlahnya
    // melebihi yang dideklarasikan pernyataan.
    let rows = if stage == "pamong" {
        c.query(sql, &[&session_id, &actor_id]).await
    } else {
        c.query(sql, &[&session_id, &actor_id, &actor_pamong]).await
    }
    .context("session_verify_list")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            scanned_at: r.get(4),
            gate_label: r.get(5),
            status: r.get(6),
        })
        .collect())
}

pub async fn decide_pamong(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    pamong_id: Option<i64>,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    // MENOLAK di sini sekalian menutup tahap final (verify_status='rejected').
    // Dulu hanya pamong_status yang diisi, verify_status dibiarkan 'pending' —
    // dan run_auto_verify_final menyetujuinya 24 jam kemudian LENGKAP DENGAN
    // POINNYA. Penolakan pamong terbatalkan diam-diam.
    //
    // Tahap 1 hanya MEMAJUKAN status; POIN diberikan sekali di tahap FINAL
    // (decide_verify / auto-verify-final) — migrasi 33. Guard: pamong bertugas
    // sesi = approver (COALESCE cs.pamong_id, cl.pamong_id); $4 NULL = admin/dewan.
    //
    // ── KENAPA ADA CTE PENARIKAN POIN ────────────────────────────────────────
    // Tahap final SENGAJA boleh mendahului pamong: antreannya menyaring
    // `pamong_status <> 'rejected'`, bukan `= 'approved'` (lihat
    // `pending_verify`). Jadi urutan guru-dulu-pamong-belakangan bukan kasus
    // aneh, melainkan alur biasa — dan konsekuensinya baris ini bisa sudah
    // `verify_status='approved'` dengan poin terlanjur masuk saat pamong
    // menolak.
    //
    // Tanpa penarikan ini santri berakhir "ditolak" tapi tetap berpoin: status
    // dan saldo saling bertentangan, dan tak ada satu pun layar yang
    // memperlihatkan pertentangan itu. Poin ditarik dengan predikat yang SAMA
    // PERSIS dengan jalur koreksi absensi (`correct_attendance`,
    // `upsert_session_attendance_bulk`) — satu cara membatalkan poin absensi,
    // bukan tiga yang mirip.
    //
    // Penghapusan cukup menyentuh `point_logs`: `trg_point_logs_balance`
    // (migrasi 32) yang mengembalikan `users.points`. Log lama sebelum migrasi
    // 51 tak punya `attendance_id` dan sengaja tak disentuh daripada salah
    // tebak.
    let c = pool.get().await?;
    let row = c
        .query_one(
            "WITH upd AS ( \
                UPDATE attendances a SET pamong_status = $2, pamong_by = $3, pamong_at = NOW(), \
                        verify_status = CASE WHEN $2 = 'rejected' THEN 'rejected' \
                                             ELSE a.verify_status END \
                 WHERE a.id = $1 AND a.pamong_status = 'pending' \
                    AND ($4::bigint IS NULL OR \
                        (SELECT COALESCE(cs.pamong_id, cl.pamong_id) FROM class_sessions cs \
                            JOIN classes cl ON cl.id = cs.class_id \
                            WHERE cs.id = a.class_session_id) = $4) \
                 RETURNING a.id \
             ), \
             tarik AS ( \
                DELETE FROM point_logs \
                 WHERE $2 = 'rejected' \
                   AND attendance_id IN (SELECT id FROM upd) \
                   AND category IN ('attendance', 'discipline') \
                RETURNING 1 \
             ) \
             SELECT count(*)::bigint FROM upd",
            &[&att_id, &status, &approver, &pamong_id],
        )
        .await
        .context("decide_pamong")?;
    let n: i64 = row.get(0);
    Ok(n > 0)
}

/// Keputusan pamong untuk BANYAK absensi sekaligus — satu pernyataan.
///
/// Bentuk SQL-nya sama persis dengan [`decide_pamong`]; yang berubah hanya
/// `a.id = $1` menjadi `a.id = ANY($1)`. Semua alasan di balik CTE penarikan
/// poin dan guard pamong bertugas berlaku identik — dibaca di sana, jangan
/// ditulis ulang di sini.
///
/// KENAPA ADA: pemanggilnya dulu mengulang [`decide_pamong`] di dalam `for`.
/// Setiap putaran mengambil koneksi pool sendiri lalu menempuh satu perjalanan
/// ke database, jadi menekan "verifikasi sesi" pada kelas 200 santri berarti
/// 200 akuisisi koneksi dan 200 perjalanan berurutan — pada pool 16 koneksi,
/// tombol itu menahan seluruh aplikasi. Dan bila koneksi putus di tengah,
/// separuh sesi terverifikasi tanpa ada yang tahu di mana berhentinya. Persis
/// alasan yang sudah ditulis untuk tahap final di [`decide_verify_bulk`];
/// tahap pamong sekadar belum menyusul.
///
/// Return jumlah baris yang benar-benar berubah — yang sudah diputus orang
/// lain lebih dulu tak ikut terhitung, tanpa menggagalkan sisanya.
pub async fn decide_pamong_bulk(
    pool: &Pool,
    att_ids: &[i64],
    approver: i64,
    approve: bool,
    pamong_id: Option<i64>,
) -> Result<i64> {
    if att_ids.is_empty() {
        return Ok(0);
    }
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let row = c
        .query_one(
            "WITH upd AS ( \
                UPDATE attendances a SET pamong_status = $2, pamong_by = $3, pamong_at = NOW(), \
                        verify_status = CASE WHEN $2 = 'rejected' THEN 'rejected' \
                                             ELSE a.verify_status END \
                 WHERE a.id = ANY($1::bigint[]) AND a.pamong_status = 'pending' \
                    AND ($4::bigint IS NULL OR \
                        (SELECT COALESCE(cs.pamong_id, cl.pamong_id) FROM class_sessions cs \
                            JOIN classes cl ON cl.id = cs.class_id \
                            WHERE cs.id = a.class_session_id) = $4) \
                 RETURNING a.id \
             ), \
             tarik AS ( \
                DELETE FROM point_logs \
                 WHERE $2 = 'rejected' \
                   AND attendance_id IN (SELECT id FROM upd) \
                   AND category IN ('attendance', 'discipline') \
                RETURNING 1 \
             ) \
             SELECT count(*)::bigint FROM upd",
            &[&att_ids, &status, &approver, &pamong_id],
        )
        .await
        .context("decide_pamong_bulk")?;
    Ok(row.get(0))
}

pub struct RiwayatRow {
    pub status: String,
    pub scanned_at: DateTime<Utc>,
    pub gate_label: Option<String>,
    /// Judul jadwal/kelas (bila absensi tertaut jadwal).
    pub title: Option<String>,
    /// Poin yang BENAR-BENAR tercatat untuk baris ini, dari buku besar.
    ///
    /// `None` = belum ada catatan poin sama sekali: absensinya belum
    /// diverifikasi, atau nilainya memang nol sehingga tak ada baris yang
    /// ditulis (`decide_verify` melewati `delta = 0`).
    pub points: Option<i32>,
}

/// Seluruh riwayat kehadiran santri (terbaru dulu) + judul kelas.
pub async fn riwayat_all(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<RiwayatRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            // Poin diambil dari `point_logs`, BUKAN ditebak dari statusnya.
            //
            // Dulu layar riwayat menghitung sendiri angkanya lewat
            // `models::point_rule` — tebakan yang tak pernah melihat buku besar.
            // Tebakan itu tak mungkin benar: potongan sesungguhnya ditentukan
            // `class_schedules.izin_points`/`late_points`/`absent_points` yang
            // BEDA-BEDA per jadwal, sementara `point_rule` hanya tahu statusnya.
            // Dan salah di layar inilah yang paling merusak kepercayaan: santri
            // membandingkan riwayatnya dengan saldonya, lalu menyimpulkan
            // aplikasinya berbohong.
            //
            // SUM, bukan satu baris: satu absensi bisa punya lebih dari satu
            // catatan (mis. potongan izin dari materialisasi lalu penyesuaian
            // pengurus). Yang ingin dilihat santri adalah dampak bersihnya.
            "SELECT a.status, a.scanned_at, a.gate_label, COALESCE(cs.title, c.name), \
                    (SELECT SUM(pl.delta)::int FROM point_logs pl \
                      WHERE pl.attendance_id = a.id) \
             FROM attendances a \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE a.user_id = $1 ORDER BY a.scanned_at DESC LIMIT $2",
            &[&user_id, &limit],
        )
        .await
        .context("riwayat_all")?;
    Ok(rows
        .into_iter()
        .map(|r| RiwayatRow {
            status: r.get(0),
            scanned_at: r.get(1),
            gate_label: r.get(2),
            title: r.get(3),
            points: r.get(4),
        })
        .collect())
}

/// Statistik semester (sejak `since`): (hadir, izin, alpa, total).
pub async fn semester_stats(
    pool: &Pool,
    user_id: i64,
    since: DateTime<Utc>,
) -> Result<(i64, i64, i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FILTER (WHERE status IN ('present','late')), \
                    COUNT(*) FILTER (WHERE status IN ('permit','sick')), \
                    COUNT(*) FILTER (WHERE status = 'absent'), \
                    COUNT(*) \
             FROM attendances WHERE user_id = $1 AND scanned_at >= $2",
            &[&user_id, &since],
        )
        .await
        .context("semester_stats")?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

/// Scan terakhir HARI INI: (judul kelas, waktu, status) — banner "Kehadiran
/// Terdeteksi" & label status hari ini di pantauan orang tua.
///
/// `status` IKUT dikembalikan (dulu tidak) karena pemanggilnya butuh membedakan
/// hadir vs terlambat; tanpa itu service/parent.rs terpaksa menebak "present"
/// dan santri yang telat tetap terlihat tepat waktu di mata orang tuanya.
pub async fn latest_scan_today(
    pool: &Pool,
    user_id: i64,
    today: NaiveDate,
) -> Result<Option<(Option<String>, DateTime<Utc>, String)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT COALESCE(cs.title, c.name), a.scanned_at, a.status \
             FROM attendances a \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE a.user_id = $1 \
               AND a.scan_date = $2 \
             ORDER BY a.scanned_at DESC LIMIT 1",
            &[&user_id, &today],
        )
        .await
        .context("latest_scan_today")?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
}

/// Hitungan bulan berjalan per-status: (hadir(present), terlambat(late), absen).
pub async fn month_counts(pool: &Pool, user_id: i64) -> Result<(i64, i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FILTER (WHERE status = 'present'), \
                    COUNT(*) FILTER (WHERE status = 'late'), \
                    COUNT(*) FILTER (WHERE status = 'absent') \
             FROM attendances \
             WHERE user_id = $1 AND scan_date >= date_trunc('month', NOW() AT TIME ZONE 'Asia/Jakarta')::date",
            &[&user_id],
        )
        .await
        .context("month_counts")?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

/// Total perubahan poin bulan berjalan (dari point_logs).
pub async fn month_points(pool: &Pool, user_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COALESCE(SUM(delta), 0)::BIGINT FROM point_logs \
             WHERE user_id = $1 AND created_at >= (date_trunc('month', NOW() AT TIME ZONE 'Asia/Jakarta') AT TIME ZONE 'Asia/Jakarta')",
            &[&user_id],
        )
        .await?;
    Ok(row.get(0))
}

/// Sudah ada absensi hari ini (per jadwal, atau scan bebas bila schedule None)?
pub async fn attendance_exists_today(
    pool: &Pool,
    user_id: i64,
    schedule_id: Option<i64>,
    today: NaiveDate,
) -> Result<bool> {
    let c = pool.get().await?;
    let row = match schedule_id {
        Some(sid) => {
            c.query_opt(
                "SELECT 1 FROM attendances \
                 WHERE user_id = $1 AND class_schedule_id = $2 \
                   AND scan_date = $3 LIMIT 1",
                &[&user_id, &sid, &today],
            )
            .await?
        }
        None => {
            c.query_opt(
                "SELECT 1 FROM attendances \
                 WHERE user_id = $1 AND class_schedule_id IS NULL \
                   AND scan_date = $2 LIMIT 1",
                &[&user_id, &today],
            )
            .await?
        }
    };
    Ok(row.is_some())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_attendance(
    pool: &Pool,
    user_id: i64,
    session_id: Option<i64>,
    schedule_id: Option<i64>,
    device_id: i64,
    gate_label: &str,
    status: &str,
    note: Option<&str>,
) -> Result<Option<i64>> {
    let c = pool.get().await?;
    // ON CONFLICT DO NOTHING: cek-lalu-insert di service TIDAK atomik, jadi dua
    // tap dalam milidetik (bounce pembaca kartu) sama-sama lolos cek lalu
    // bertabrakan di UNIQUE. Tanpa ini tabrakan jadi HTTP 500 ke mesin →
    // firmware retry → alarm Telegram palsu. None = sudah tercatat, bukan galat.
    let row = c
        .query_opt(
            "INSERT INTO attendances \
             (user_id, class_session_id, class_schedule_id, device_id, gate_label, status, method, note, scan_date) \
             VALUES ($1, $2, $3, $4, $5, $6, 'rfid', $7, (NOW() AT TIME ZONE 'Asia/Jakarta')::date) \
             ON CONFLICT DO NOTHING \
             RETURNING id",
            &[&user_id, &session_id, &schedule_id, &device_id, &gate_label, &status, &note],
        )
        .await
        .context("insert_attendance")?;
    Ok(row.map(|r| r.get(0)))
}

// ── Verifikasi TAHAP FINAL (USTAD bertugas sesi) ─────────────────────────────────

/// Antrean tahap final: menunggu verifikasi USTAD bertugas sesi. 2 langkah
/// (require_pamong) → harus lolos pamong dulu; 1 langkah → langsung. `teacher_id`
/// Some = hanya sesi yang ustadnya guru ini (COALESCE cs.teacher_id,
/// cl.wali_kelas_id); None = semua (dewan guru/admin oversight). Migrasi 33.
pub async fn pending_verify(
    pool: &Pool,
    teacher_id: Option<i64>,
    limit: i64,
) -> Result<Vec<PendingRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT a.id, u.full_name, u.nis, cl.name, a.scanned_at, a.gate_label, a.status \
             FROM attendances a \
             JOIN users u ON u.id = a.user_id \
             LEFT JOIN class_sessions cs ON cs.id = a.class_session_id \
             LEFT JOIN classes cl ON cl.id = cs.class_id \
             WHERE a.verify_status = 'pending' \
                AND a.pamong_status <> 'rejected' \
                AND ($2::bigint IS NULL OR COALESCE(cs.teacher_id, cl.wali_kelas_id) = $2) \
             ORDER BY a.scanned_at ASC LIMIT $1",
            &[&limit, &teacher_id],
        )
        .await
        .context("pending_verify")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            scanned_at: r.get(4),
            gate_label: r.get(5),
            status: r.get(6),
        })
        .collect())
}

/// Jumlah terverifikasi final hari ini.
pub async fn verified_today(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            &format!(
                "SELECT COUNT(*) FROM attendances \
                 WHERE verify_status = 'approved' AND {hari}",
                hari = super::hari_ini_wib("verified_at"),
            ),
            &[],
        )
        .await?;
    Ok(row.get(0))
}

/// Delta poin satu baris absensi — SATU sumber kebenaran aritmetika poin.
///
/// Pemanggil wajib menyediakan dua alias: `att` (baris `attendances`, dipakai
/// kolom `status`) dan `sch` (hasil LEFT JOIN ke `class_schedules`, boleh NULL
/// bila absensi tak terikat jadwal). Nilai override di `class_schedules`
/// disimpan sebagai MAGNITUDO POSITIF (CHECK migrasi 21 & 44); tanda minus
/// ditentukan di sini, bukan di data.
///
/// Kenapa satu string SQL, bukan fungsi Rust: aturan ini dibutuhkan di dua
/// jalur yang keduanya berjalan utuh di dalam SATU pernyataan SQL —
/// [`decide_verify`] (verifikasi manual) dan [`run_auto_verify_final`]
/// (verifikasi otomatis) — plus fungsi `cat_default_points()` migrasi 28 yang
/// juga dipakai trigger saldo migrasi 32. Sebelumnya jalur manual menghitung
/// delta di Rust (`models::attendance_delta`) sedangkan jalur otomatis
/// menghitungnya di SQL: aturan bisnis yang sama hidup di dua bahasa, dan
/// keduanya harus diingat saat PRD berubah.
///
/// Kedua salinan itu sempat berbeda tanpa ketahuan, dan yang menutupinya cuma
/// CHECK constraint: cabang terakhir versi SQL dulu `ELSE -absent` (status tak
/// dikenal → penalti penuh) sedangkan Rust `_ => 0` (netral). Aman hanya karena
/// migrasi 7 mengunci `status` ke enam nilai. Di bawah ini `absent` ditulis
/// eksplisit dan `ELSE` dikembalikan ke 0 — status yang belum dikenal tak
/// seharusnya diam-diam kena potongan terbesar.
///
/// ── IZIN: SATU SUMBER, `class_schedules.izin_points` ─────────────────────────
/// Nilai `permit` sempat ditulis 0 mati di sini untuk mengakhiri pertentangan
/// dengan `models::point_rule` yang menampilkan 0 ke santri. Yang tak disadari
/// saat itu: pertentangannya tidak hilang, hanya berpindah. Jalur
/// `permits::materialize_permit_attendance` TETAP memotong `−izin_points`, jadi
/// dua santri dengan izin serupa mendapat perlakuan berbeda semata karena
/// barisnya lahir dari jalur yang berbeda. Komentar lama di sini bahkan
/// menyatakan `class_schedules.izin_points` "tak lagi dibaca siapa pun" —
/// tidak benar, dan justru membuat orang berhenti mencari.
///
/// Sekarang KEDUA jalur membaca kolom yang sama, `class_schedules.izin_points`
/// (migrasi 28) — kolom yang memang sudah ada untuk keperluan ini dan sudah
/// bisa disunting dari form jadwal. Tak perlu setelan baru di tempat lain.
///
/// ATURANNYA: potongan berlaku HANYA bila `izin_points` diisi dan lebih dari 0.
/// NULL berarti tidak memotong — BUKAN jatuh ke preset PRD seperti tiga kolom
/// poin lainnya. Perbedaan yang disengaja: hadir/telat/alfa punya nilai wajar
/// yang berlaku di kegiatan mana pun, sedangkan "apakah izin dihukum" adalah
/// kebijakan yang berbeda-beda per kegiatan dan tak pantas ditebak preset.
/// Diam berarti tidak memotong; yang memotong harus dinyatakan.
///
/// Karena letaknya di JADWAL, keputusannya bisa berbeda antar kegiatan dalam
/// satu kelas yang sama — ngaji wajib hadir, kajian tambahan tidak — sesuatu
/// yang setelan per-kelas tak bisa ungkapkan.
///
/// `izin_points` dijaga `>= 0` oleh CHECK migrasi 44, jadi "tidak null DAN
/// lebih dari 0" cukup ditulis `COALESCE(sch.izin_points, 0)`.
const DELTA_SQL: &str = "CASE \
     WHEN att.status = 'present' THEN COALESCE(sch.present_points, cat_default_points(COALESCE(sch.activity_type,'other'),'present'))::int \
     WHEN att.status = 'late' THEN -COALESCE(sch.late_points, cat_default_points(COALESCE(sch.activity_type,'other'),'late'))::int \
     WHEN att.status = 'permit' THEN -COALESCE(sch.izin_points, 0)::int \
     WHEN att.status = 'absent' THEN -COALESCE(sch.absent_points, cat_default_points(COALESCE(sch.activity_type,'other'),'absent'))::int \
     ELSE 0 \
   END";

/// Verifikasi FINAL oleh ustad bertugas (migrasi 33). Guard: ustad sesi =
/// approver (teacher_id Some; None = dewan/admin). Prereq: 2 langkah → pamong
/// approved; 1 langkah → langsung. POIN diberikan DI SINI (sekali, saat final)
/// utk KEDUA mode → hindari dobel dgn trigger (migrasi 32). Return true bila
/// ada baris ter-update.
/// `actor_pamong` — lihat [`session_verify_list`]: penentu apakah `actor_id`
/// diadu dengan guru sesi atau pamong sesi. Daftar dan keputusan HARUS memakai
/// syarat yang sama, kalau tidak orang bisa memutuskan baris yang tak pernah
/// boleh ia lihat.
/// Verifikasi final BANYAK absensi sekaligus — satu pernyataan, satu transaksi.
///
/// Versi satuannya dipanggil sekali per santri oleh tombol "verifikasi sesi":
/// satu sesi 200 santri berarti 200 transaksi dan 400 perjalanan ke database
/// (UPDATE + INSERT poin masing-masing). Di sini keduanya jadi satu pernyataan
/// ber-CTE, bentuk yang sama dengan jalur verifikasi otomatis
/// (`run_auto_verify_final`) supaya angka poinnya mustahil berbeda.
///
/// Syarat kepemilikan sama persis dengan versi satuan — lihat `decide_verify`.
/// Baris yang tak lolos syarat diam-diam dilewati, bukan menggagalkan yang
/// lain: pemanggilnya mengirim seluruh antrean sekaligus, dan satu baris yang
/// sudah keburu diverifikasi orang lain tak boleh membatalkan sisanya.
///
/// Return jumlah baris yang benar-benar berubah.
pub async fn decide_verify_bulk(
    pool: &Pool,
    att_ids: &[i64],
    approver: i64,
    approve: bool,
    actor_id: Option<i64>,
    actor_pamong: bool,
) -> Result<i64> {
    if att_ids.is_empty() {
        return Ok(0);
    }
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;

    // Ditolak → cukup UPDATE, tak ada poin yang diberikan.
    if !approve {
        let n = c
            .execute(
                "UPDATE attendances a SET verify_status = $2, verified_by = $3, verified_at = NOW() \
                 WHERE a.id = ANY($1::bigint[]) AND a.verify_status = 'pending' \
                    AND a.pamong_status <> 'rejected' \
                    AND ($4::bigint IS NULL OR \
                        (SELECT CASE WHEN $5::bool THEN COALESCE(cs.pamong_id, cl.pamong_id) \
                                     ELSE COALESCE(cs.teacher_id, cl.wali_kelas_id) END \
                           FROM class_sessions cs \
                            JOIN classes cl ON cl.id = cs.class_id \
                           WHERE cs.id = a.class_session_id) = $4)",
                &[&att_ids, &status, &approver, &actor_id, &actor_pamong],
            )
            .await
            .context("decide_verify_bulk tolak")?;
        return Ok(n as i64);
    }

    let sql = format!(
        "WITH upd AS ( \
            UPDATE attendances a SET verify_status = 'approved', verified_by = $2, \
                                     verified_at = NOW() \
             WHERE a.id = ANY($1::bigint[]) AND a.verify_status = 'pending' \
                AND a.pamong_status <> 'rejected' \
                AND ($3::bigint IS NULL OR \
                    (SELECT CASE WHEN $4::bool THEN COALESCE(cs.pamong_id, cl.pamong_id) \
                                 ELSE COALESCE(cs.teacher_id, cl.wali_kelas_id) END \
                       FROM class_sessions cs \
                        JOIN classes cl ON cl.id = cs.class_id \
                       WHERE cs.id = a.class_session_id) = $3) \
             RETURNING a.id, a.user_id, a.status, a.class_schedule_id \
         ), \
         pts AS ( \
            SELECT att.id AS att_id, att.user_id, att.status, {DELTA_SQL} AS delta \
              FROM upd att \
              LEFT JOIN class_schedules sch ON sch.id = att.class_schedule_id \
         ), \
         lg AS ( \
            INSERT INTO point_logs \
                (user_id, delta, reason, category, given_by, attendance_id) \
            SELECT p.user_id, p.delta, \
                   'Kehadiran (' || p.status || ') — ' || \
                   CASE p.status \
                        WHEN 'present' THEN 'Kedisiplinan' \
                        WHEN 'late'    THEN 'Kedisiplinan' \
                        WHEN 'absent'  THEN 'Pelanggaran' \
                        WHEN 'permit'  THEN 'Izin' \
                        WHEN 'sick'    THEN 'Sakit/Cuti' \
                        ELSE 'Keterangan' END, \
                   CASE WHEN p.delta < 0 THEN 'discipline' ELSE 'attendance' END, \
                   $2, p.att_id \
              FROM pts p WHERE p.delta <> 0 \
            RETURNING 1 \
         ) \
         SELECT count(*)::bigint FROM upd"
    );
    let row = c
        .query_one(&sql, &[&att_ids, &approver, &actor_id, &actor_pamong])
        .await
        .context("decide_verify_bulk setujui")?;
    Ok(row.get(0))
}

pub async fn decide_verify(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    actor_id: Option<i64>,
    actor_pamong: bool,
) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("decide_verify tx")?;
    let status = if approve { "approved" } else { "rejected" };
    let row = tx
        .query_opt(
            "UPDATE attendances a SET verify_status = $2, verified_by = $3, verified_at = NOW() \
             WHERE a.id = $1 AND a.verify_status = 'pending' \
                AND a.pamong_status <> 'rejected' \
                AND ($4::bigint IS NULL OR \
                    (SELECT CASE WHEN $5::bool THEN COALESCE(cs.pamong_id, cl.pamong_id) \
                                 ELSE COALESCE(cs.teacher_id, cl.wali_kelas_id) END \
                       FROM class_sessions cs \
                        JOIN classes cl ON cl.id = cs.class_id WHERE cs.id = a.class_session_id) = $4) \
             RETURNING a.user_id, a.status, a.class_schedule_id",
            &[&att_id, &status, &approver, &actor_id, &actor_pamong],
        )
        .await
        .context("decide_verify update")?;

    let Some(row) = row else {
        tx.rollback().await.ok();
        return Ok(false);
    };

    if approve {
        let att_status: String = row.get(1);
        // Rust hanya menyumbang KATA-nya; angkanya dihitung SQL lewat DELTA_SQL
        // supaya identik dengan jalur verifikasi otomatis.
        let reason = format!(
            "Kehadiran ({att_status}) — {}",
            crate::models::attendance_note(&att_status)
        );
        // Baris absensinya dibaca ulang di dalam transaksi yang sama (UPDATE di
        // atas sudah terlihat), lalu di-join ke jadwalnya supaya override poin
        // per-jadwal ikut terpakai.
        //
        // `category` diturunkan dari TANDA delta, sama seperti jalur otomatis.
        // Ini setara dengan pemetaan per-status yang dulu dipakai di sini:
        // present selalu ≥ 0 → attendance; late/absent/permit selalu ≤ 0 →
        // discipline; sick & outside_schedule berdelta 0 dan sudah disaring
        // `delta <> 0`, jadi tak pernah sampai menghasilkan baris log.
        //
        // attendance_id: tautan agar koreksi bisa MENARIK BALIK poin ini
        // (hapus log → trigger mengembalikan saldo). Migrasi 51.
        // users.points sendiri diperbarui otomatis oleh trigger (migrasi 32).
        let sql = format!(
            "INSERT INTO point_logs (user_id, delta, reason, category, given_by, attendance_id) \
             SELECT att.user_id, d.delta, $2, \
                    CASE WHEN d.delta < 0 THEN 'discipline' ELSE 'attendance' END, \
                    $3, att.id \
             FROM attendances att \
             LEFT JOIN class_schedules sch ON sch.id = att.class_schedule_id \
             CROSS JOIN LATERAL (SELECT {DELTA_SQL} AS delta) d \
             WHERE att.id = $1 AND d.delta <> 0"
        );
        tx.execute(&sql, &[&att_id, &reason, &approver])
            .await
            .context("decide_verify point_logs")?;
    }

    tx.commit().await.context("decide_verify commit")?;
    Ok(true)
}

// ── Auto-absent (job penutup sesi) ───────────────────────────────────────────────

/// Tandai ABSENT — "Alpa" (satu query set-based) untuk santri terdaftar yang
/// TIDAK ada kejelasan (bukan hadir/terlambat, bukan izin disetujui) pada sesi
/// yang sudah TUNTAS. Auto-verified (pamong+dewan guru = approved, oleh sistem)
/// + penalti poin langsung, agar tak membanjiri antrean verifikasi manusia &
/// query analisa/report tinggal filter `status='absent'` (tak perlu hitung
/// "tak ada baris" secara terpisah). Poin yang dipotong per santri mengikuti
/// `class_schedules.absent_points` bila diisi (migrasi 15) — beda jadwal beda
/// bobot pelanggaran — atau default global 15 bila NULL. Nilai kolom SELALU
/// magnitude positif; dihitung sebagai `points = points - absent_points`.
///
/// Sesi TUNTAS = tanggalnya sudah lewat (hari sebelumnya atau lebih lama), ATAU
/// hari ini tapi jam selesai jadwalnya sudah lewat. Sesi TANPA jadwal (ad-hoc,
/// `class_schedule_id` NULL — sebelumnya TERLEWAT total krn INNER JOIN jadwal)
/// kini ikut tercakup lewat cabang "tanggal sudah lewat" (tak ada jam acuan utk
/// hari yg sama → baru ditandai besoknya, aman drpd menembak terlalu dini) —
/// keanggotaan sesi ad-hoc diambil dari SELURUH peserta kelas (cp.class_id),
/// bukan salah satu jadwal spesifik. DIBATASI 3 hari ke belakang (jangan sampai
/// downtime lama tiba-tiba menghukum retroaktif riwayat lama). DIKECUALIKAN:
/// sesi libur (cancelled), santri yg sudah punya catatan pada sesi itu, dan
/// santri dgn izin (permit_requests) disetujui yang mencakup TANGGAL SESI itu.
/// Idempotent BERLAPIS DUA. `NOT EXISTS` menyaring yang sudah tercatat saat
/// query disusun; `ON CONFLICT DO NOTHING` menangkap yang lolos di antara
/// pembacaan dan penulisan. Komentar lama di sini menyatakan "tak ada UNIQUE
/// constraint di attendances" — itu belum benar sejak migrasi 42, yang justru
/// memasang dua index unik untuk aturan dedup ini.
///
/// Tanpa lapis kedua, dua eksekusi yang tumpang tindih (job harian tertunda +
/// pemicuan manual, atau dua instance server) membuat INSERT kedua menabrak
/// UNIQUE, dan karena seluruhnya SATU pernyataan, SEMUA baris alpa hari itu
/// ikut batal — bukan cuma yang bentrok. Kegagalan diam-diam yang menghukum
/// hari berikutnya, bukan hari ini.
///
/// `ON CONFLICT` sengaja TANPA target. Baris yang ditulis di sini bisa
/// menabrak DUA constraint berbeda yang dua-duanya berarti "sudah ada
/// catatan": `UNIQUE (user_id, class_session_id)` (migrasi 2) dan
/// `uq_attendance_schedule_daily` (migrasi 42, parsial). Satu klausa bertarget
/// hanya bisa menyebut salah satunya, dan yang tak disebut tetap menggagalkan
/// seluruh pernyataan. Ini pengecualian sadar dari kebiasaan "selalu sebut
/// targetnya" — di sini targetnya memang jamak.
///
/// Return jumlah baris alpa baru.
///
/// Izin memerdekakan santri HANYA untuk kelas yang izinnya ditujukan
/// (`p.class_id`). Sejak migrasi 46 izin dipecah per kelas, jadi tanpa syarat
/// ini satu izin untuk kelas A ikut melindungi santri di kelas B pada hari yang
/// sama — dia bolos kelas B tanpa tercatat. Izin lama tanpa class_id (NULL)
/// tetap berlaku menyeluruh, seperti sebelum migrasi 46.
/// Kunci penasihat (advisory lock) job auto-absent. Angkanya sembarang tapi
/// TETAP — yang penting tak dipakai job lain. Dicatat di sini, bukan di
/// pemanggil, supaya siapa pun yang menambah job baru melihat nomor yang sudah
/// terpakai saat membaca berkas ini.
const LOCK_AUTO_ABSENT: i64 = 0x7070_6D5F_6162_7301; // "ppm_abs" + 01
pub async fn run_auto_absent(pool: &Pool) -> Result<i64> {
    let mut c = pool.get().await?;
    // Kunci TRANSAKSIONAL, bukan `pg_try_advisory_lock` biasa: koneksi datang
    // dari pool dan dikembalikan setelah dipakai. Kunci sesi akan ikut menempel
    // pada koneksi yang didaur ulang, dan job berikutnya menemukan kunci yang
    // tak pernah dilepas siapa pun. Versi `_xact_` lepas sendiri saat COMMIT
    // atau ROLLBACK, termasuk saat prosesnya mati di tengah.
    let tx = c.transaction().await.context("run_auto_absent transaction")?;
    let dapat: bool = tx
        .query_one("SELECT pg_try_advisory_xact_lock($1)", &[&LOCK_AUTO_ABSENT])
        .await
        .context("run_auto_absent lock")?
        .get(0);
    if !dapat {
        // Eksekusi lain sedang berjalan. Menunggu tak ada gunanya — yang
        // sedang jalan akan menandai baris yang sama — jadi lebih baik pulang
        // dengan angka nol daripada menumpuk koneksi di belakang kunci.
        tracing::info!("run_auto_absent dilewati: eksekusi lain sedang berjalan");
        return Ok(0);
    }
    let row = tx
        .query_one(
            "WITH tz AS (SELECT (NOW() AT TIME ZONE 'Asia/Jakarta') AS n), \
             ins AS ( \
                INSERT INTO attendances \
                    (user_id, class_session_id, class_schedule_id, status, method, \
                     pamong_status, pamong_at, verify_status, verified_at, note, gate_label, scanned_at, scan_date) \
                SELECT DISTINCT cp.user_id, s.id, s.class_schedule_id, 'absent', 'manual', \
                       'approved', NOW(), 'approved', NOW(), 'Auto: tidak hadir', 'system', NOW(), s.session_date \
                FROM class_sessions s \
                LEFT JOIN class_schedules sch ON sch.id = s.class_schedule_id \
                JOIN class_participants cp ON cp.class_id = s.class_id \
                CROSS JOIN tz \
                WHERE s.status <> 'cancelled' \
                  AND s.session_date >= (tz.n)::date - INTERVAL '3 days' \
                  AND ( \
                        s.session_date < (tz.n)::date \
                        OR (s.session_date = (tz.n)::date AND sch.end_time IS NOT NULL \
                            AND sch.end_time < (tz.n)::time) \
                      ) \
                  AND NOT EXISTS (SELECT 1 FROM attendances a \
                        WHERE a.user_id = cp.user_id AND a.class_session_id = s.id) \
                  AND NOT EXISTS (SELECT 1 FROM attendances a2 \
                        WHERE a2.user_id = cp.user_id AND a2.class_schedule_id = s.class_schedule_id \
                          AND a2.scan_date = s.session_date) \
                  AND NOT EXISTS (SELECT 1 FROM permit_requests p \
                        WHERE p.user_id = cp.user_id \
                          AND p.guru_status = 'approved' \
                          AND p.start_date <= s.session_date \
                          AND COALESCE(p.end_date, p.start_date) >= s.session_date \
                          AND (p.class_id IS NULL \
                               OR EXISTS (SELECT 1 FROM permit_request_classes prc \
                                           WHERE prc.permit_id = p.id \
                                             AND prc.class_id = s.class_id)) \
                          AND (p.start_time IS NULL \
                               OR (sch.start_time < p.end_time \
                                   AND sch.end_time > p.start_time))) \
                ON CONFLICT DO NOTHING \
                RETURNING id, user_id, class_schedule_id \
             ), \
             lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category, attendance_id) \
                SELECT ins.user_id, \
                       -COALESCE(sch.absent_points, cat_default_points(COALESCE(sch.activity_type,'other'),'absent'))::int, \
                       'Kehadiran (absent) — otomatis', 'discipline', ins.id \
                FROM ins \
                LEFT JOIN class_schedules sch ON sch.id = ins.class_schedule_id \
                RETURNING user_id \
             ) \
             SELECT COUNT(*)::bigint FROM ins",
            &[],
        )
        .await
        .context("run_auto_absent")?;
    let n: i64 = row.get(0);
    tx.commit().await.context("run_auto_absent commit")?;
    Ok(n)
}

// ── Auto-verify (queue tak disentuh manusia >1 hari) ─────────────────────────

/// Auto-approve TAHAP 1 (pamong) untuk absensi yang sudah >1 hari menunggu
/// sejak `scanned_at` tanpa keputusan manual (satu query set-based, sama
/// filosofi `run_auto_absent` — jangan biarkan antrean menumpuk selamanya).
///
/// TIDAK memberi poin. Sejak migrasi 33 poin absensi ditulis SEKALI di tahap
/// FINAL (`decide_verify` / `run_auto_verify_final`); tahap pamong hanya
/// memajukan status. Doc lama di sini menyebut "present +10, late +2" — angka
/// yang tak pernah lagi ditulis fungsi ini, dan menyesatkan justru di jalur
/// yang paling sulit diperiksa ulang.
///
/// `pamong_by` dibiarkan NULL (oleh sistem, bukan pengguna — pola sama
/// `run_auto_absent`). Return jumlah baris.
pub async fn run_auto_verify_pamong(pool: &Pool) -> Result<i64> {
    // Hanya MEMAJUKAN tahap pamong utk kelas 2-langkah (require_pamong) yg
    // menunggu >1 hari. TANPA poin (poin diberikan di tahap FINAL, migrasi 33).
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE attendances a SET pamong_status = 'approved', pamong_at = NOW() \
             WHERE a.pamong_status = 'pending' \
               AND a.scanned_at < NOW() - INTERVAL '1 day' \
               AND COALESCE( \
                   (SELECT cl.require_pamong FROM class_sessions cs \
                       JOIN classes cl ON cl.id = cs.class_id WHERE cs.id = a.class_session_id), TRUE) = TRUE",
            &[],
        )
        .await
        .context("run_auto_verify_pamong")?;
    Ok(n as i64)
}

/// Auto-verifikasi FINAL utk absensi yg menunggu >1 hari (2-langkah: pamong
/// sudah approved & pamong_at lawas; 1-langkah: scanned_at lawas). POIN diberikan
/// DI SINI (sekali, sama seperti decide_verify) — migrasi 33. Return jumlah baris.
pub async fn run_auto_verify_final(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    // Ekspresi delta-nya SATU dengan jalur manual (lihat DELTA_SQL) — alias
    // `att`/`sch` di bawah dipilih agar cocok dengan yang diharapkannya.
    let sql = format!(
        "WITH upd AS ( \
            UPDATE attendances a SET verify_status = 'approved', verified_at = NOW() \
            WHERE a.verify_status = 'pending' \
              AND a.pamong_status <> 'rejected' \
              AND a.scanned_at < NOW() - INTERVAL '1 day' \
            RETURNING a.id, a.user_id, a.status, a.class_schedule_id \
         ), \
         pts AS ( \
            SELECT att.id AS att_id, att.user_id, att.status, {DELTA_SQL} AS delta \
            FROM upd att \
            LEFT JOIN class_schedules sch ON sch.id = att.class_schedule_id \
         ), \
         lg AS ( \
            INSERT INTO point_logs (user_id, delta, reason, category, attendance_id) \
            SELECT user_id, delta, \
                   'Kehadiran (' || status || ') — verifikasi final otomatis', \
                   CASE WHEN delta < 0 THEN 'discipline' ELSE 'attendance' END, \
                   att_id \
            FROM pts WHERE delta <> 0 \
            RETURNING user_id \
         ) \
         SELECT COUNT(*)::bigint FROM upd"
    );
    let row = c
        .query_one(&sql, &[])
        .await
        .context("run_auto_verify_final")?;
    Ok(row.get(0))
}

/// Koreksi status absensi oleh GURU PENGISI atau PAMONG yang bertugas di sesi
/// itu — dan hanya mereka.
///
/// Kenapa sesempit itu: yang tahu apa yang sebenarnya terjadi di ruangan
/// hanyalah orang yang ada di sana. Admin bisa saja salah menebak, dan membuka
/// koreksi ke semua staf berarti catatan kehadiran bisa diubah siapa pun tanpa
/// konteks.
///
/// Poin lama DITARIK dengan menghapus `point_logs` yang tertaut (migrasi 51) —
/// trigger `trg_point_logs_balance` mengembalikan saldonya sendiri, tanpa
/// aritmetika manual yang bisa salah. Poin BARU tidak langsung diberikan:
/// baris dikembalikan ke antrean verifikasi final supaya jalur pemberian poin
/// tetap satu pintu.
///
/// Return false = tak ditemukan, status sama, atau pemanggil bukan petugas sesi.
pub async fn correct_attendance(
    pool: &Pool,
    att_id: i64,
    new_status: &str,
    actor_id: i64,
) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("correct_attendance tx")?;

    // Penjagaan ADA DI QUERY, bukan cuma di service: pemanggil harus guru
    // pengisi atau pamong sesi ini (dengan fallback ke wali/pamong kelas bila
    // sesi belum menetapkan petugasnya).
    let updated = tx
        .execute(
            "UPDATE attendances a \
                SET status = $2, \
                    verify_status = 'pending', \
                    corrected_by = $3, \
                    corrected_at = NOW() \
              WHERE a.id = $1 \
                AND a.status <> $2 \
                AND EXISTS ( \
                      SELECT 1 FROM class_sessions cs \
                        JOIN classes cl ON cl.id = cs.class_id \
                       WHERE cs.id = a.class_session_id \
                         AND $3 IN (COALESCE(cs.teacher_id, cl.wali_kelas_id), \
                                    COALESCE(cs.pamong_id, cl.pamong_id)) \
                    )",
            &[&att_id, &new_status, &actor_id],
        )
        .await
        .context("correct_attendance update")?;

    if updated == 0 {
        tx.rollback().await.ok();
        return Ok(false);
    }

    // Tarik poin dari status LAMA. Hanya log yang benar-benar tertaut ke baris
    // ini — log lama (sebelum migrasi 51) tak punya tautan dan sengaja tak
    // disentuh daripada salah tebak.
    tx.execute(
        "DELETE FROM point_logs WHERE attendance_id = $1 \
           AND category IN ('attendance', 'discipline')",
        &[&att_id],
    )
    .await
    .context("correct_attendance rollback poin")?;

    tx.commit().await.context("correct_attendance commit")?;
    Ok(true)
}

/// Petugas sesi sebuah absensi: (guru pengisi, pamong). Dipakai UI untuk
/// menentukan apakah tombol koreksi perlu ditampilkan.
pub async fn attendance_session_staff(
    pool: &Pool,
    att_id: i64,
) -> Result<Option<(Option<i64>, Option<i64>)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT COALESCE(cs.teacher_id, cl.wali_kelas_id), \
                    COALESCE(cs.pamong_id, cl.pamong_id) \
               FROM attendances a \
               JOIN class_sessions cs ON cs.id = a.class_session_id \
               JOIN classes cl ON cl.id = cs.class_id \
              WHERE a.id = $1",
            &[&att_id],
        )
        .await
        .context("attendance_session_staff")?;
    Ok(row.map(|r| (r.get(0), r.get(1))))
}

/// Info sesi yang dibutuhkan koreksi massal + PENJAGAAN petugas dalam satu
/// query: `None` = sesi tak ada ATAU pemanggil bukan guru/pamong sesi itu.
///
/// COALESCE ke wali/pamong KELAS: bila sesi belum menetapkan petugasnya,
/// yang berlaku adalah wali kelas & pamong kelas — jadi sesi yang terlanjur
/// lewat tanpa penugasan tetap bisa dikoreksi.
pub async fn session_for_correction(
    pool: &Pool,
    session_id: i64,
    actor_id: i64,
) -> Result<Option<(Option<i64>, NaiveDate, Option<chrono::NaiveTime>)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cs.class_schedule_id, cs.session_date, sch.limit_entery_time \
             FROM class_sessions cs \
             JOIN classes cl ON cl.id = cs.class_id \
             LEFT JOIN class_schedules sch ON sch.id = cs.class_schedule_id \
             WHERE cs.id = $1 \
               AND $2 IN (COALESCE(cs.teacher_id, cl.wali_kelas_id), \
                          COALESCE(cs.pamong_id, cl.pamong_id))",
            &[&session_id, &actor_id],
        )
        .await
        .context("session_for_correction")?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
}

/// Set status (dan opsional JAM masuk) satu santri di sebuah sesi.
/// INSERT bila belum ada catatan, UPDATE bila sudah.
///
/// Selalu mengembalikan baris ke antrean verifikasi (`verify_status='pending'`)
/// dan MENARIK poin lama yang tertaut — sama seperti koreksi tunggal, supaya
/// mengubah status lewat jalur massal tak menyisakan poin dari status lama.
///
/// Pemanggil WAJIB sudah memverifikasi kewenangan lewat
/// [`session_for_correction`]; fungsi ini tak memeriksanya lagi agar
/// penjagaannya tidak berulang untuk tiap santri dalam satu koreksi massal.
/// Koreksi absensi BANYAK santri dalam SATU pernyataan.
///
/// Versi satuannya (`upsert_session_attendance`) dipanggil dalam loop: satu
/// sesi berisi 200 santri berarti 200 kali ambil-koneksi, 200 transaksi, dan
/// 200 perjalanan bolak-balik ke database — untuk satu tombol "simpan". Di sini
/// seluruhnya jadi satu transaksi dengan dua pernyataan set-based.
///
/// Bentuk masukan sengaja tiga larik sejajar (user_id, status, jam) alih-alih
/// larik struct: itu yang bisa diterima `unnest` tanpa tipe komposit buatan.
///
/// Pagar keanggotaan kelas tetap sama dengan versi satuan — `JOIN
/// class_participants` menjatuhkan id yang bukan anggota kelas sesi ini, jadi
/// daftar dari klien tak bisa menyelundupkan santri kelas lain.
///
/// Return jumlah baris yang benar-benar tersimpan.
pub async fn upsert_session_attendance_bulk(
    pool: &Pool,
    session_id: i64,
    schedule_id: Option<i64>,
    session_date: NaiveDate,
    user_ids: &[i64],
    statuses: &[String],
    jams: &[Option<chrono::NaiveTime>],
    actor_id: i64,
) -> Result<i64> {
    if user_ids.is_empty() {
        return Ok(0);
    }
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("upsert_bulk tx")?;

    let rows = tx
        .query(
            "WITH masuk AS ( \
                SELECT * FROM unnest($4::bigint[], $5::text[], $6::time[]) \
                            AS t(uid, status, jam) \
             ), \
             ins AS ( \
                INSERT INTO attendances \
                    (user_id, class_session_id, class_schedule_id, gate_label, status, method, \
                     note, scan_date, scanned_at) \
                SELECT cp.user_id, $1, $2, 'manual', m.status, 'manual', 'diisi petugas', $3, \
                       COALESCE(($3::date + m.jam) AT TIME ZONE 'Asia/Jakarta', NOW()) \
                  FROM masuk m \
                  JOIN class_sessions s ON s.id = $1 \
                  JOIN class_participants cp \
                       ON cp.class_id = s.class_id AND cp.user_id = m.uid \
                ON CONFLICT (user_id, class_session_id) DO UPDATE \
                   SET status = EXCLUDED.status, \
                       verify_status = 'pending', \
                       corrected_by = $7, \
                       corrected_at = NOW(), \
                       scanned_at = COALESCE(EXCLUDED.scanned_at, attendances.scanned_at) \
                RETURNING id \
             ) \
             SELECT id FROM ins",
            &[
                &session_id,
                &schedule_id,
                &session_date,
                &user_ids,
                &statuses,
                &jams,
                &actor_id,
            ],
        )
        .await
        .context("upsert_bulk insert")?;

    let ids: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
    if ids.is_empty() {
        tx.rollback().await.ok();
        return Ok(0);
    }

    // Tarik poin dari status LAMA untuk SEMUA baris sekaligus (migrasi 51).
    tx.execute(
        "DELETE FROM point_logs WHERE attendance_id = ANY($1::bigint[]) \
           AND category IN ('attendance', 'discipline')",
        &[&ids],
    )
    .await
    .context("upsert_bulk tarik poin")?;

    tx.commit().await.context("upsert_bulk commit")?;
    Ok(ids.len() as i64)
}

pub async fn upsert_session_attendance(
    pool: &Pool,
    session_id: i64,
    schedule_id: Option<i64>,
    session_date: NaiveDate,
    user_id: i64,
    status: &str,
    jam: Option<chrono::NaiveTime>,
    actor_id: i64,
) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("upsert_attendance tx")?;

    // Jam diisi → scanned_at disusun dari TANGGAL SESI + jam itu (WIB), bukan
    // waktu sekarang: petugas lazim mengisi absensi setelah sesi usai.
    let row = tx
        .query_opt(
            // SELECT ... FROM class_participants, bukan VALUES: keanggotaan
            // kelas jadi syarat baris ini ada sama sekali. Koreksi massal
            // menerima daftar user_id dari klien, dan tanpa pagar ini satu
            // request bisa menuliskan kehadiran + poin atas nama siapa pun —
            // termasuk santri kelas lain atau staf.
            "INSERT INTO attendances \
                (user_id, class_session_id, class_schedule_id, gate_label, status, method, \
                 note, scan_date, scanned_at) \
             SELECT cp.user_id, $2, $3, 'manual', $4, 'manual', 'diisi petugas', $5, \
                    COALESCE(($5::date + $6::time) AT TIME ZONE 'Asia/Jakarta', NOW()) \
             FROM class_sessions s \
             JOIN class_participants cp ON cp.class_id = s.class_id AND cp.user_id = $1 \
             WHERE s.id = $2 \
             ON CONFLICT (user_id, class_session_id) DO UPDATE \
                SET status = EXCLUDED.status, \
                    verify_status = 'pending', \
                    corrected_by = $7, \
                    corrected_at = NOW(), \
                    scanned_at = COALESCE(($5::date + $6::time) AT TIME ZONE 'Asia/Jakarta', \
                                          attendances.scanned_at) \
             RETURNING id",
            &[
                &user_id, &session_id, &schedule_id, &status, &session_date, &jam, &actor_id,
            ],
        )
        .await
        .context("upsert_attendance")?;

    // Tak ada baris = user itu bukan anggota kelas sesi ini (atau sesinya tak
    // ada). Dilaporkan sebagai false, bukan galat: pemanggil sudah menghitung
    // "berapa yang tersimpan", dan koreksi massal tak boleh gagal seluruhnya
    // hanya karena satu id nyasar.
    let Some(row) = row else {
        tx.rollback().await.ok();
        tracing::warn!(session_id, user_id, "koreksi absensi ditolak: bukan anggota kelas");
        return Ok(false);
    };
    let att_id: i64 = row.get(0);
    // Tarik poin dari status LAMA (log yang tertaut baris ini, migrasi 51).
    tx.execute(
        "DELETE FROM point_logs WHERE attendance_id = $1 \
           AND category IN ('attendance', 'discipline')",
        &[&att_id],
    )
    .await
    .context("upsert_attendance tarik poin")?;

    tx.commit().await.context("upsert_attendance commit")?;
    Ok(true)
}
