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
    let rows = c
        .query(
            "SELECT u.full_name, u.nis, cl.name, SUM(pl.delta)::int AS net \
             FROM point_logs pl \
             JOIN users u ON u.id = pl.user_id AND u.role IN ('santri', 'santri_finance') \
             LEFT JOIN class_participants cp ON cp.user_id = u.id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE (pl.created_at AT TIME ZONE 'Asia/Jakarta')::date BETWEEN $1 AND $2 \
             GROUP BY u.id, u.full_name, u.nis, cl.name \
             HAVING SUM(pl.delta) <= -9 \
             ORDER BY net ASC",
            &[&start, &end],
        )
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
                AND (a.scanned_at AT TIME ZONE 'Asia/Jakarta')::date BETWEEN $1 AND $2 \
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
                    WHERE cp.user_id = u.id ORDER BY cp.is_primary DESC LIMIT 1), \
                COUNT(*) FILTER (WHERE a.status = 'present'), \
                COUNT(*) FILTER (WHERE a.status IN ('late','outside_schedule')), \
                COUNT(*) FILTER (WHERE a.status IN ('permit','sick')), \
                COUNT(*) FILTER (WHERE a.status = 'absent'), \
                u.points \
             FROM users u \
             LEFT JOIN attendances a ON a.user_id = u.id \
                AND (a.scanned_at AT TIME ZONE 'Asia/Jakarta')::date BETWEEN $1 AND $2 \
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
             WHERE user_id = $1 AND scanned_at >= date_trunc('month', NOW())",
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
            "SELECT COUNT(*) FROM attendances \
             WHERE pamong_status = 'approved' \
               AND (pamong_at AT TIME ZONE 'Asia/Jakarta')::date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date",
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
pub async fn session_verify_list(
    pool: &Pool,
    session_id: i64,
    stage: &str,
    actor_id: Option<i64>,
) -> Result<Vec<PendingRow>> {
    let c = pool.get().await?;
    let sql = if stage == "pamong" {
        "SELECT a.id, u.full_name, u.nis, cl.name, a.scanned_at, a.gate_label, a.status \
         FROM attendances a JOIN users u ON u.id = a.user_id \
         LEFT JOIN class_sessions cs ON cs.id = a.class_session_id \
         LEFT JOIN classes cl ON cl.id = cs.class_id \
         WHERE a.class_session_id = $1 AND a.pamong_status = 'pending' \
           AND COALESCE(cl.require_pamong, TRUE) = TRUE \
           AND ($2::bigint IS NULL OR COALESCE(cs.pamong_id, cl.pamong_id) = $2) \
         ORDER BY u.full_name"
    } else {
        "SELECT a.id, u.full_name, u.nis, cl.name, a.scanned_at, a.gate_label, a.status \
         FROM attendances a JOIN users u ON u.id = a.user_id \
         LEFT JOIN class_sessions cs ON cs.id = a.class_session_id \
         LEFT JOIN classes cl ON cl.id = cs.class_id \
         WHERE a.class_session_id = $1 AND a.verify_status = 'pending' \
           AND CASE WHEN COALESCE(cl.require_pamong, TRUE) \
                THEN a.pamong_status = 'approved' ELSE TRUE END \
           AND ($2::bigint IS NULL OR COALESCE(cs.teacher_id, cl.wali_kelas_id) = $2) \
         ORDER BY u.full_name"
    };
    let rows = c
        .query(sql, &[&session_id, &actor_id])
        .await
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
    // Tahap 1 hanya MEMAJUKAN status; POIN diberikan sekali di tahap FINAL
    // (decide_verify / auto-verify-final) — migrasi 33. Guard: pamong bertugas
    // sesi = approver (COALESCE cs.pamong_id, cl.pamong_id); $4 NULL = admin/dewan.
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE attendances a SET pamong_status = $2, pamong_by = $3, pamong_at = NOW() \
             WHERE a.id = $1 AND a.pamong_status = 'pending' \
                AND ($4::bigint IS NULL OR \
                    (SELECT COALESCE(cs.pamong_id, cl.pamong_id) FROM class_sessions cs \
                        JOIN classes cl ON cl.id = cs.class_id \
                        WHERE cs.id = a.class_session_id) = $4) \
             RETURNING a.id",
            &[&att_id, &status, &approver, &pamong_id],
        )
        .await
        .context("decide_pamong")?;
    Ok(n > 0)
}

pub struct RiwayatRow {
    pub status: String,
    pub scanned_at: DateTime<Utc>,
    pub gate_label: Option<String>,
    /// Judul jadwal/kelas (bila absensi tertaut jadwal).
    pub title: Option<String>,
}

/// Seluruh riwayat kehadiran santri (terbaru dulu) + judul kelas.
pub async fn riwayat_all(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<RiwayatRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT a.status, a.scanned_at, a.gate_label, COALESCE(cs.title, c.name) \
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

/// Scan terakhir HARI INI (judul kelas + waktu) — banner "Kehadiran Terdeteksi".
pub async fn latest_scan_today(
    pool: &Pool,
    user_id: i64,
    today: NaiveDate,
) -> Result<Option<(Option<String>, DateTime<Utc>)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT COALESCE(cs.title, c.name), a.scanned_at \
             FROM attendances a \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE a.user_id = $1 \
               AND (a.scanned_at AT TIME ZONE 'Asia/Jakarta')::date = $2 \
             ORDER BY a.scanned_at DESC LIMIT 1",
            &[&user_id, &today],
        )
        .await
        .context("latest_scan_today")?;
    Ok(row.map(|r| (r.get(0), r.get(1))))
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
             WHERE user_id = $1 AND scanned_at >= date_trunc('month', NOW())",
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
             WHERE user_id = $1 AND created_at >= date_trunc('month', NOW())",
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
                   AND (scanned_at AT TIME ZONE 'Asia/Jakarta')::date = $3 LIMIT 1",
                &[&user_id, &sid, &today],
            )
            .await?
        }
        None => {
            c.query_opt(
                "SELECT 1 FROM attendances \
                 WHERE user_id = $1 AND class_schedule_id IS NULL \
                   AND (scanned_at AT TIME ZONE 'Asia/Jakarta')::date = $2 LIMIT 1",
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
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO attendances \
             (user_id, class_session_id, class_schedule_id, device_id, gate_label, status, method, note, scan_date) \
             VALUES ($1, $2, $3, $4, $5, $6, 'rfid', $7, (NOW() AT TIME ZONE 'Asia/Jakarta')::date) \
             RETURNING id",
            &[&user_id, &session_id, &schedule_id, &device_id, &gate_label, &status, &note],
        )
        .await
        .context("insert_attendance")?;
    Ok(row.get(0))
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
                AND CASE WHEN COALESCE(cl.require_pamong, TRUE) \
                         THEN a.pamong_status = 'approved' ELSE TRUE END \
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
            "SELECT COUNT(*) FROM attendances \
             WHERE verify_status = 'approved' \
               AND (verified_at AT TIME ZONE 'Asia/Jakarta')::date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date",
            &[],
        )
        .await?;
    Ok(row.get(0))
}

/// Verifikasi FINAL oleh ustad bertugas (migrasi 33). Guard: ustad sesi =
/// approver (teacher_id Some; None = dewan/admin). Prereq: 2 langkah → pamong
/// approved; 1 langkah → langsung. POIN diberikan DI SINI (sekali, saat final)
/// utk KEDUA mode → hindari dobel dgn trigger (migrasi 32). Return true bila
/// ada baris ter-update.
pub async fn decide_verify(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    teacher_id: Option<i64>,
) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("decide_verify tx")?;
    let status = if approve { "approved" } else { "rejected" };
    let row = tx
        .query_opt(
            "UPDATE attendances a SET verify_status = $2, verified_by = $3, verified_at = NOW() \
             WHERE a.id = $1 AND a.verify_status = 'pending' \
                AND CASE WHEN COALESCE( \
                        (SELECT cl.require_pamong FROM class_sessions cs \
                            JOIN classes cl ON cl.id = cs.class_id WHERE cs.id = a.class_session_id), TRUE) \
                          AND (SELECT COALESCE(cs.pamong_id, cl.pamong_id) FROM class_sessions cs \
                                JOIN classes cl ON cl.id = cs.class_id \
                                WHERE cs.id = a.class_session_id) IS NOT NULL \
                     THEN a.pamong_status = 'approved' ELSE TRUE END \
                AND ($4::bigint IS NULL OR \
                    (SELECT COALESCE(cs.teacher_id, cl.wali_kelas_id) FROM class_sessions cs \
                        JOIN classes cl ON cl.id = cs.class_id WHERE cs.id = a.class_session_id) = $4) \
             RETURNING a.user_id, a.status, a.class_schedule_id",
            &[&att_id, &status, &approver, &teacher_id],
        )
        .await
        .context("decide_verify update")?;

    let Some(row) = row else {
        tx.rollback().await.ok();
        return Ok(false);
    };

    if approve {
        let user_id: i64 = row.get(0);
        let att_status: String = row.get(1);
        let schedule_id: Option<i64> = row.get(2);
        let (mut present_p, mut late_p, mut absent_p, mut izin_p) = (None, None, None, None);
        let mut activity_type = String::new();
        if let Some(sid) = schedule_id {
            if let Some(r) = tx
                .query_opt(
                    "SELECT present_points, late_points, absent_points, izin_points, \
                            COALESCE(activity_type, '') FROM class_schedules WHERE id = $1",
                    &[&sid],
                )
                .await
                .context("decide_verify schedule points")?
            {
                present_p = r.get(0);
                late_p = r.get(1);
                absent_p = r.get(2);
                izin_p = r.get(3);
                activity_type = r.get(4);
            }
        }
        let (delta, note, category) = crate::models::attendance_delta(
            &att_status, &activity_type, present_p, late_p, absent_p, izin_p,
        );
        if delta != 0 {
            let reason = format!("Kehadiran ({att_status}) — {note}");
            // users.points diperbarui otomatis oleh trigger (migrasi 32).
            tx.execute(
                "INSERT INTO point_logs (user_id, delta, reason, category, given_by) \
                 VALUES ($1, $2, $3, $4, $5)",
                &[&user_id, &delta, &reason, &category, &approver],
            )
            .await
            .context("decide_verify point_logs")?;
        }
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
/// Idempotent via NOT EXISTS (tak ada UNIQUE constraint di attendances —
/// lihat roadmap #6). Return jumlah baris alpa baru.
pub async fn run_auto_absent(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
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
                JOIN class_participants cp \
                    ON (s.class_schedule_id IS NOT NULL AND cp.class_schedule_id = s.class_schedule_id) \
                    OR (s.class_schedule_id IS NULL AND cp.class_id = s.class_id) \
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
                          AND (a2.scanned_at AT TIME ZONE 'Asia/Jakarta')::date = s.session_date) \
                  AND NOT EXISTS (SELECT 1 FROM permit_requests p \
                        WHERE p.user_id = cp.user_id \
                          AND p.guru_status = 'approved' \
                          AND p.start_date <= s.session_date \
                          AND COALESCE(p.end_date, p.start_date) >= s.session_date) \
                RETURNING id, user_id, class_schedule_id \
             ), \
             lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category) \
                SELECT ins.user_id, \
                       -COALESCE(sch.absent_points, cat_default_points(COALESCE(sch.activity_type,'other'),'absent'))::int, \
                       'Kehadiran (absent) — otomatis', 'discipline' \
                FROM ins \
                LEFT JOIN class_schedules sch ON sch.id = ins.class_schedule_id \
                RETURNING user_id \
             ) \
             SELECT COUNT(*)::bigint FROM ins",
            &[],
        )
        .await
        .context("run_auto_absent")?;
    Ok(row.get(0))
}

// ── Auto-verify (queue tak disentuh manusia >1 hari) ─────────────────────────

/// Auto-approve TAHAP 1 (pamong) untuk absensi yang sudah >1 hari menunggu
/// sejak `scanned_at` tanpa keputusan manual (satu query set-based, sama
/// filosofi `run_auto_absent` — jangan biarkan antrean menumpuk selamanya).
/// Poin diberikan SAMA seperti `decide_pamong` approve manual: present +10,
/// late +2 (atau override `class_schedules.late_points` bila diisi),
/// outside_schedule/permit/sick netral. `pamong_by` dibiarkan NULL (oleh
/// sistem, bukan pengguna — pola sama `run_auto_absent`). Return jumlah baris.
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
    let row = c
        .query_one(
            "WITH upd AS ( \
                UPDATE attendances a SET verify_status = 'approved', verified_at = NOW() \
                WHERE a.verify_status = 'pending' \
                  AND CASE WHEN COALESCE( \
                          (SELECT cl.require_pamong FROM class_sessions cs \
                              JOIN classes cl ON cl.id = cs.class_id WHERE cs.id = a.class_session_id), TRUE) \
                            AND (SELECT COALESCE(cs.pamong_id, cl.pamong_id) FROM class_sessions cs \
                                  JOIN classes cl ON cl.id = cs.class_id \
                                  WHERE cs.id = a.class_session_id) IS NOT NULL \
                       THEN a.pamong_status = 'approved' AND a.pamong_at < NOW() - INTERVAL '1 day' \
                       ELSE a.scanned_at < NOW() - INTERVAL '1 day' END \
                RETURNING a.user_id, a.status, a.class_schedule_id \
             ), \
             pts AS ( \
                SELECT upd.user_id, upd.status, \
                       CASE \
                         WHEN upd.status = 'present' THEN COALESCE(sch.present_points, cat_default_points(COALESCE(sch.activity_type,'other'),'present'))::int \
                         WHEN upd.status = 'late' THEN -COALESCE(sch.late_points, cat_default_points(COALESCE(sch.activity_type,'other'),'late'))::int \
                         WHEN upd.status IN ('sick','outside_schedule') THEN 0 \
                         WHEN upd.status = 'permit' THEN -COALESCE(sch.izin_points, cat_default_points(COALESCE(sch.activity_type,'other'),'izin'))::int \
                         ELSE -COALESCE(sch.absent_points, cat_default_points(COALESCE(sch.activity_type,'other'),'absent'))::int \
                       END AS delta \
                FROM upd \
                LEFT JOIN class_schedules sch ON sch.id = upd.class_schedule_id \
             ), \
             lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category) \
                SELECT user_id, delta, \
                       'Kehadiran (' || status || ') — verifikasi final otomatis', \
                       CASE WHEN delta < 0 THEN 'discipline' ELSE 'attendance' END \
                FROM pts WHERE delta <> 0 \
                RETURNING user_id \
             ) \
             SELECT COUNT(*)::bigint FROM upd",
            &[],
        )
        .await
        .context("run_auto_verify_final")?;
    Ok(row.get(0))
}
