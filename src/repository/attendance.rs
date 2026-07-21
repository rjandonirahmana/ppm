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

/// Antrean verifikasi pamong (pamong_status = pending), tertua dulu.
pub async fn pending_pamong(pool: &Pool, limit: i64) -> Result<Vec<PendingRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT a.id, u.full_name, u.nis, c.name, a.scanned_at, a.gate_label, a.status \
             FROM attendances a \
             JOIN users u ON u.id = a.user_id \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE a.pamong_status = 'pending' \
             ORDER BY a.scanned_at ASC LIMIT $1",
            &[&limit],
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
             WHERE pamong_status = 'approved' AND pamong_at >= date_trunc('day', NOW())",
            &[],
        )
        .await?;
    Ok(row.get(0))
}

/// Setujui/tolak (tahap pamong) DALAM SATU TRANSAKSI. Saat disetujui, poin
/// kehadiran diberikan (models::attendance::point_rule): insert point_logs +
/// update saldo users.points. Return true bila ada baris ter-update.
pub async fn decide_pamong(pool: &Pool, att_id: i64, approver: i64, approve: bool) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("decide_pamong tx")?;

    let status = if approve { "approved" } else { "rejected" };
    let row = tx
        .query_opt(
            "UPDATE attendances SET pamong_status = $2, pamong_by = $3, pamong_at = NOW() \
             WHERE id = $1 AND pamong_status = 'pending' \
             RETURNING user_id, status, class_schedule_id",
            &[&att_id, &status, &approver],
        )
        .await
        .context("decide_pamong update")?;

    let Some(row) = row else {
        tx.rollback().await.ok();
        return Ok(false);
    };

    if approve {
        let user_id: i64 = row.get(0);
        let att_status: String = row.get(1);
        let schedule_id: Option<i64> = row.get(2);
        let (mut delta, mut note, category) = crate::models::point_rule(&att_status);
        // Poin TERLAMBAT bisa dikustomisasi per jadwal (mis. sholat = -5,
        // pengajian = tetap default) — lihat migrasi 13. Hanya berlaku utk
        // status 'late'; status lain tetap pakai aturan global.
        if att_status == "late" {
            if let Some(sid) = schedule_id {
                let custom: Option<i16> = tx
                    .query_opt("SELECT late_points FROM class_schedules WHERE id = $1", &[&sid])
                    .await
                    .context("decide_pamong late_points")?
                    .and_then(|r| r.get(0));
                if let Some(lp) = custom {
                    delta = lp as i32;
                    note = "Kedisiplinan (kustom jadwal)";
                }
            }
        }
        if delta != 0 {
            let reason = format!("Kehadiran ({att_status}) — {note}");
            tx.execute(
                "INSERT INTO point_logs (user_id, delta, reason, category, given_by) \
                 VALUES ($1, $2, $3, $4, $5)",
                &[&user_id, &delta, &reason, &category, &approver],
            )
            .await
            .context("decide_pamong point_logs")?;
            tx.execute(
                "UPDATE users SET points = points + $2 WHERE id = $1",
                &[&user_id, &delta],
            )
            .await
            .context("decide_pamong points")?;
        }
    }

    tx.commit().await.context("decide_pamong commit")?;
    Ok(true)
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
             (user_id, class_session_id, class_schedule_id, device_id, gate_label, status, method, note) \
             VALUES ($1, $2, $3, $4, $5, $6, 'rfid', $7) RETURNING id",
            &[&user_id, &session_id, &schedule_id, &device_id, &gate_label, &status, &note],
        )
        .await
        .context("insert_attendance")?;
    Ok(row.get(0))
}

// ── Verifikasi TAHAP 2 (dewan guru — final) ──────────────────────────────────────

/// Antrean tahap 2: sudah disetujui pamong, menunggu verifikasi final dewan guru.
pub async fn pending_verify(pool: &Pool, limit: i64) -> Result<Vec<PendingRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT a.id, u.full_name, u.nis, c.name, a.scanned_at, a.gate_label, a.status \
             FROM attendances a \
             JOIN users u ON u.id = a.user_id \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE a.pamong_status = 'approved' AND a.verify_status = 'pending' \
             ORDER BY a.scanned_at ASC LIMIT $1",
            &[&limit],
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
             WHERE verify_status = 'approved' AND verified_at >= date_trunc('day', NOW())",
            &[],
        )
        .await?;
    Ok(row.get(0))
}

/// Verifikasi final (tahap 2). Poin TIDAK diubah (sudah diberikan tahap pamong);
/// ini hanya menyetel verify_status final. Return true bila ada baris ter-update.
pub async fn decide_verify(pool: &Pool, att_id: i64, approver: i64, approve: bool) -> Result<bool> {
    let c = pool.get().await?;
    let status = if approve { "approved" } else { "rejected" };
    let n = c
        .execute(
            "UPDATE attendances SET verify_status = $2, verified_by = $3, verified_at = NOW() \
             WHERE id = $1 AND pamong_status = 'approved' AND verify_status = 'pending'",
            &[&att_id, &status, &approver],
        )
        .await
        .context("decide_verify")?;
    Ok(n > 0)
}

// ── Auto-absent (job penutup sesi) ───────────────────────────────────────────────

/// Tandai ABSENT — "Alpa" (satu query set-based) untuk santri terdaftar yang
/// TIDAK ada kejelasan (bukan hadir/terlambat, bukan izin disetujui) pada sesi
/// yang sudah TUNTAS. Auto-verified (pamong+dewan guru = approved, oleh sistem)
/// + penalti poin langsung, agar tak membanjiri antrean verifikasi manusia &
/// query analisa/report tinggal filter `status='absent'` (tak perlu hitung
/// "tak ada baris" secara terpisah).
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
                     pamong_status, pamong_at, verify_status, verified_at, note, gate_label, scanned_at) \
                SELECT DISTINCT cp.user_id, s.id, s.class_schedule_id, 'absent', 'manual', \
                       'approved', NOW(), 'approved', NOW(), 'Auto: tidak hadir', 'system', NOW() \
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
                        WHERE p.user_id = cp.user_id AND p.status = 'approved' \
                          AND p.start_date <= s.session_date \
                          AND COALESCE(p.end_date, p.start_date) >= s.session_date) \
                RETURNING id, user_id \
             ), \
             lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category) \
                SELECT user_id, -15, 'Kehadiran (absent) — otomatis', 'discipline' FROM ins \
             ), \
             agg AS (SELECT user_id, COUNT(*)::int AS n FROM ins GROUP BY user_id), \
             upd AS ( \
                UPDATE users u SET points = points - (agg.n * 15) \
                FROM agg WHERE u.id = agg.user_id RETURNING u.id \
             ) \
             SELECT COUNT(*)::bigint FROM ins",
            &[],
        )
        .await
        .context("run_auto_absent")?;
    Ok(row.get(0))
}
