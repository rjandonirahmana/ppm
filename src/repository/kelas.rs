//! repository/kelas.rs — Query agregat sisi STAF/GURU/DEWAN GURU: dashboard
//! staf, ranking kelas, insight guru, papan poin santri.
//!
//! Semua query di sini SCOPED lewat parameter `teacher_id: Option<i64>`:
//! `None` = seluruh pesantren (admin/dewan guru), `Some(id)` = hanya
//! kelas-kelas yang sesi terakhirnya diampu guru tsb (role teacher biasa).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

// ── Dashboard staf ───────────────────────────────────────────────────────────────

/// (total_santri, santri_baru_bulan_ini, hadir_hari_ini, izin_pending).
pub async fn staf_stats(pool: &Pool) -> Result<(i64, i64, i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT \
                (SELECT COUNT(*) FROM users WHERE role = 'santri' AND is_active = TRUE), \
                (SELECT COUNT(*) FROM users WHERE role = 'santri' AND is_active = TRUE \
                    AND created_at >= date_trunc('month', NOW())), \
                (SELECT COUNT(*) FROM attendances a JOIN users u ON u.id = a.user_id \
                    WHERE u.role = 'santri' AND a.status IN ('present','late') \
                    AND (a.scanned_at AT TIME ZONE 'Asia/Jakarta')::date = CURRENT_DATE), \
                (SELECT COUNT(*) FROM permit_requests WHERE status = 'pending')",
            &[],
        )
        .await
        .context("staf_stats")?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub struct LiveSesiRow {
    pub title: String,
    pub teacher: String,
    pub santri_count: i64,
    pub state: String,
    pub time_label: Option<chrono::NaiveTime>,
}

/// Sesi kelas hari ini (berlangsung + akan datang), untuk kartu "Sesi Live".
pub async fn today_sessions(pool: &Pool, limit: i64) -> Result<Vec<LiveSesiRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT COALESCE(s.title, cs.title, c.name), COALESCE(t.full_name, 'Belum ditentukan'), \
                    (SELECT COUNT(*) FROM class_participants cp WHERE cp.class_id = c.id), \
                    s.status, cs.start_time \
             FROM class_sessions s \
             JOIN classes c ON c.id = s.class_id \
             LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
             LEFT JOIN users t ON t.id = s.teacher_id \
             WHERE s.session_date = CURRENT_DATE \
             ORDER BY CASE s.status WHEN 'ongoing' THEN 0 WHEN 'scheduled' THEN 1 ELSE 2 END, \
                      cs.start_time ASC NULLS LAST \
             LIMIT $1",
            &[&limit],
        )
        .await
        .context("today_sessions")?;
    Ok(rows
        .into_iter()
        .map(|r| LiveSesiRow {
            title: r.get(0),
            teacher: r.get(1),
            santri_count: r.get(2),
            state: r.get(3),
            time_label: r.get(4),
        })
        .collect())
}

pub struct LatestAttRow {
    pub name: String,
    pub class_name: Option<String>,
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

/// Kehadiran terbaru (semua santri) — untuk tabel "Kehadiran Terbaru" staf.
pub async fn latest_attendance(pool: &Pool, limit: i64) -> Result<Vec<LatestAttRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, COALESCE(cs.title, c.name), a.scanned_at, a.status \
             FROM attendances a \
             JOIN users u ON u.id = a.user_id \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE u.role = 'santri' \
             ORDER BY a.scanned_at DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("latest_attendance")?;
    Ok(rows
        .into_iter()
        .map(|r| LatestAttRow {
            name: r.get(0),
            class_name: r.get(1),
            scanned_at: r.get(2),
            status: r.get(3),
        })
        .collect())
}

// ── Analisis (guru / dewan guru) ─────────────────────────────────────────────────

/// (pct_kehadiran, rata2_poin, sesi_terverifikasi) — dalam cakupan 30 hari
/// terakhir. `teacher_id = None` → seluruh pesantren.
pub async fn analisis_summary(pool: &Pool, teacher_id: Option<i64>) -> Result<(i32, i32, i64)> {
    let c = pool.get().await?;
    let row = match teacher_id {
        None => {
            c.query_one(
                "SELECT \
                    COALESCE(ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(*), 0)), 0)::INT, \
                    COALESCE((SELECT ROUND(AVG(points)) FROM users WHERE role = 'santri'), 0)::INT, \
                    (SELECT COUNT(*) FROM attendances WHERE pamong_status = 'approved' \
                        AND pamong_at >= NOW() - INTERVAL '30 days') \
                 FROM attendances a JOIN users u ON u.id = a.user_id \
                 WHERE u.role = 'santri' AND a.scanned_at >= NOW() - INTERVAL '30 days'",
                &[],
            )
            .await
        }
        Some(tid) => {
            c.query_one(
                "SELECT \
                    COALESCE(ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(*), 0)), 0)::INT, \
                    COALESCE((SELECT ROUND(AVG(u2.points)) FROM users u2 \
                        JOIN class_participants cp ON cp.user_id = u2.id \
                        WHERE u2.role = 'santri' AND cp.class_id IN \
                            (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1)), 0)::INT, \
                    (SELECT COUNT(*) FROM attendances a2 \
                        JOIN class_sessions s2 ON s2.id = a2.class_session_id \
                        WHERE s2.teacher_id = $1 AND a2.pamong_status = 'approved' \
                        AND a2.pamong_at >= NOW() - INTERVAL '30 days') \
                 FROM attendances a \
                 JOIN class_sessions s ON s.id = a.class_session_id \
                 WHERE s.teacher_id = $1 AND a.scanned_at >= NOW() - INTERVAL '30 days'",
                &[&tid],
            )
            .await
        }
    }
    .context("analisis_summary")?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

/// Tren kehadiran 7 hari terakhir (persentase per hari).
pub async fn attendance_trend_7d(
    pool: &Pool,
    teacher_id: Option<i64>,
) -> Result<Vec<(chrono::NaiveDate, i32)>> {
    let c = pool.get().await?;
    let rows = match teacher_id {
        None => {
            c.query(
                "SELECT d::date, COALESCE(( \
                    SELECT ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) / NULLIF(COUNT(*), 0)) \
                    FROM attendances a JOIN users u ON u.id = a.user_id \
                    WHERE u.role = 'santri' AND (a.scanned_at AT TIME ZONE 'Asia/Jakarta')::date = d::date \
                 ), 0)::INT \
                 FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, INTERVAL '1 day') d",
                &[],
            )
            .await
        }
        Some(tid) => {
            c.query(
                "SELECT d::date, COALESCE(( \
                    SELECT ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) / NULLIF(COUNT(*), 0)) \
                    FROM attendances a JOIN class_sessions s ON s.id = a.class_session_id \
                    WHERE s.teacher_id = $1 AND (a.scanned_at AT TIME ZONE 'Asia/Jakarta')::date = d::date \
                 ), 0)::INT \
                 FROM generate_series(CURRENT_DATE - INTERVAL '6 days', CURRENT_DATE, INTERVAL '1 day') d",
                &[&tid],
            )
            .await
        }
    }
    .context("attendance_trend_7d")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

pub struct ClassRankRow {
    pub name: String,
    pub attendance_pct: i32,
    pub avg_points: i32,
    pub santri_count: i64,
}

/// Ranking kelas berdasar persentase kehadiran 30 hari terakhir.
pub async fn class_ranking(
    pool: &Pool,
    teacher_id: Option<i64>,
    limit: i64,
) -> Result<Vec<ClassRankRow>> {
    let c = pool.get().await?;
    let rows = match teacher_id {
        None => {
            c.query(
                "SELECT c.name, \
                    COALESCE(ROUND(100.0 * COUNT(a.*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(a.*), 0)), 0)::INT, \
                    COALESCE(ROUND(AVG(u.points)), 0)::INT, \
                    COUNT(DISTINCT cp.user_id) \
                 FROM classes c \
                 LEFT JOIN class_participants cp ON cp.class_id = c.id \
                 LEFT JOIN users u ON u.id = cp.user_id AND u.role = 'santri' \
                 LEFT JOIN class_schedules cs ON cs.class_id = c.id \
                 LEFT JOIN attendances a ON a.class_schedule_id = cs.id \
                    AND a.scanned_at >= NOW() - INTERVAL '30 days' \
                 GROUP BY c.id, c.name ORDER BY 2 DESC LIMIT $1",
                &[&limit],
            )
            .await
        }
        Some(tid) => {
            c.query(
                "SELECT c.name, \
                    COALESCE(ROUND(100.0 * COUNT(a.*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(a.*), 0)), 0)::INT, \
                    COALESCE(ROUND(AVG(u.points)), 0)::INT, \
                    COUNT(DISTINCT cp.user_id) \
                 FROM classes c \
                 JOIN class_sessions s ON s.class_id = c.id AND s.teacher_id = $1 \
                 LEFT JOIN class_participants cp ON cp.class_id = c.id \
                 LEFT JOIN users u ON u.id = cp.user_id AND u.role = 'santri' \
                 LEFT JOIN attendances a ON a.class_session_id = s.id \
                 GROUP BY c.id, c.name ORDER BY 2 DESC LIMIT $2",
                &[&tid, &limit],
            )
            .await
        }
    }
    .context("class_ranking")?;
    Ok(rows
        .into_iter()
        .map(|r| ClassRankRow {
            name: r.get(0),
            attendance_pct: r.get(1),
            avg_points: r.get(2),
            santri_count: r.get(3),
        })
        .collect())
}

pub struct TeacherInsightRow {
    pub name: String,
    pub sessions_count: i64,
    pub attendance_pct: i32,
}

/// Insight kinerja pengajar (dewan guru saja — semua guru).
pub async fn teacher_insight(pool: &Pool, limit: i64) -> Result<Vec<TeacherInsightRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT t.full_name, COUNT(DISTINCT s.id), \
                COALESCE(ROUND(100.0 * COUNT(a.*) FILTER (WHERE a.status IN ('present','late')) \
                    / NULLIF(COUNT(a.*), 0)), 0)::INT \
             FROM users t \
             JOIN class_sessions s ON s.teacher_id = t.id \
             LEFT JOIN attendances a ON a.class_session_id = s.id \
             WHERE t.role = 'teacher' AND s.session_date >= CURRENT_DATE - INTERVAL '30 days' \
             GROUP BY t.id, t.full_name ORDER BY 3 DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("teacher_insight")?;
    Ok(rows
        .into_iter()
        .map(|r| TeacherInsightRow {
            name: r.get(0),
            sessions_count: r.get(1),
            attendance_pct: r.get(2),
        })
        .collect())
}

// ── Poin santri ───────────────────────────────────────────────────────────────────

pub struct PointRowDb {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub points: i32,
}

/// Papan poin santri, terurut (desc = tertinggi dulu).
pub async fn points_board(
    pool: &Pool,
    teacher_id: Option<i64>,
    limit: i64,
    desc: bool,
) -> Result<Vec<PointRowDb>> {
    let c = pool.get().await?;
    let order = if desc { "DESC" } else { "ASC" };
    let rows = match teacher_id {
        None => {
            let sql = format!(
                "SELECT u.id, u.full_name, u.nis, \
                    (SELECT c.name FROM class_participants cp JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = u.id LIMIT 1), \
                    u.points \
                 FROM users u WHERE u.role = 'santri' AND u.is_active = TRUE \
                 ORDER BY u.points {order} LIMIT $1"
            );
            c.query(&sql, &[&limit]).await
        }
        Some(tid) => {
            let sql = format!(
                "SELECT u.id, u.full_name, u.nis, c.name, u.points \
                 FROM users u \
                 JOIN class_participants cp ON cp.user_id = u.id \
                 JOIN classes c ON c.id = cp.class_id \
                 WHERE u.role = 'santri' AND u.is_active = TRUE \
                   AND cp.class_id IN (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1) \
                 ORDER BY u.points {order} LIMIT $2"
            );
            c.query(&sql, &[&tid, &limit]).await
        }
    }
    .context("points_board")?;
    Ok(rows
        .into_iter()
        .map(|r| PointRowDb {
            user_id: r.get(0),
            name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            points: r.get(4),
        })
        .collect())
}

/// (rata-rata poin, jumlah santri) dalam cakupan.
pub async fn points_avg(pool: &Pool, teacher_id: Option<i64>) -> Result<(i32, i64)> {
    let c = pool.get().await?;
    let row = match teacher_id {
        None => {
            c.query_one(
                "SELECT COALESCE(ROUND(AVG(points)), 0)::INT, COUNT(*) \
                 FROM users WHERE role = 'santri' AND is_active = TRUE",
                &[],
            )
            .await
        }
        Some(tid) => {
            c.query_one(
                "SELECT COALESCE(ROUND(AVG(u.points)), 0)::INT, COUNT(DISTINCT u.id) \
                 FROM users u JOIN class_participants cp ON cp.user_id = u.id \
                 WHERE u.role = 'santri' AND u.is_active = TRUE \
                   AND cp.class_id IN (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1)",
                &[&tid],
            )
            .await
        }
    }
    .context("points_avg")?;
    Ok((row.get(0), row.get(1)))
}

/// Tambah/kurangi poin manual (dewan guru/admin) + catat di point_logs.
pub async fn adjust_points(
    pool: &Pool,
    user_id: i64,
    delta: i32,
    reason: &str,
    given_by: i64,
) -> Result<()> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("adjust_points tx")?;
    tx.execute(
        "INSERT INTO point_logs (user_id, delta, reason, category, given_by) \
         VALUES ($1, $2, $3, 'manual', $4)",
        &[&user_id, &delta, &reason, &given_by],
    )
    .await
    .context("adjust_points insert")?;
    tx.execute(
        "UPDATE users SET points = points + $2 WHERE id = $1",
        &[&user_id, &delta],
    )
    .await
    .context("adjust_points update")?;
    tx.commit().await.context("adjust_points commit")?;
    Ok(())
}

// ── Manajemen Kelas (admin/dewan guru/pamong) ────────────────────────────────────

pub struct ClassListRow {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub teacher: String,
    pub member_count: i64,
    pub schedule_count: i64,
}

/// Daftar kelas aktif + agregat (anggota unik, jumlah jadwal, pengajar terakhir).
pub async fn list_classes(pool: &Pool) -> Result<Vec<ClassListRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT c.id, c.name, COALESCE(c.description, ''), c.category, \
                COALESCE((SELECT t.full_name FROM class_sessions s JOIN users t ON t.id = s.teacher_id \
                    WHERE s.class_id = c.id AND s.teacher_id IS NOT NULL \
                    ORDER BY s.session_date DESC LIMIT 1), '-'), \
                (SELECT COUNT(DISTINCT cp.user_id) FROM class_participants cp WHERE cp.class_id = c.id), \
                (SELECT COUNT(*) FROM class_schedules cs WHERE cs.class_id = c.id) \
             FROM classes c WHERE c.status = 'active' ORDER BY c.created_at DESC",
            &[],
        )
        .await
        .context("list_classes")?;
    Ok(rows
        .into_iter()
        .map(|r| ClassListRow {
            id: r.get(0),
            name: r.get(1),
            description: r.get(2),
            category: r.get(3),
            teacher: r.get(4),
            member_count: r.get(5),
            schedule_count: r.get(6),
        })
        .collect())
}

/// Kategori kelas yang sudah dipakai (DISTINCT) — untuk dropdown + ketik baru.
pub async fn distinct_categories(pool: &Pool) -> Result<Vec<String>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT category FROM classes \
             WHERE category IS NOT NULL AND category <> '' ORDER BY category",
            &[],
        )
        .await
        .context("distinct_categories")?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}

/// Ubah kelas (nama + kategori). category kosong → NULL.
pub async fn update_class(
    pool: &Pool,
    class_id: i64,
    name: &str,
    category: Option<&str>,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE classes SET name = $2, category = $3, updated_at = NOW() WHERE id = $1",
            &[&class_id, &name, &category],
        )
        .await
        .context("update_class")?;
    Ok(n > 0)
}

/// (total_kelas_aktif, total_santri_aktif).
pub async fn class_totals(pool: &Pool) -> Result<(i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT (SELECT COUNT(*) FROM classes WHERE status = 'active'), \
                    (SELECT COUNT(*) FROM users WHERE role = 'santri' AND is_active = TRUE)",
            &[],
        )
        .await
        .context("class_totals")?;
    Ok((row.get(0), row.get(1)))
}

/// Buat kelas baru (nama + kategori opsional) → id.
pub async fn create_class(
    pool: &Pool,
    name: &str,
    category: Option<&str>,
    description: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO classes (name, category, description) VALUES ($1, $2, $3) RETURNING id",
            &[&name, &category, &description],
        )
        .await
        .context("create_class")?;
    Ok(row.get(0))
}

/// Info dasar kelas (nama, deskripsi, kategori).
pub async fn class_info(
    pool: &Pool,
    class_id: i64,
) -> Result<Option<(String, String, Option<String>)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT name, COALESCE(description, ''), category FROM classes WHERE id = $1",
            &[&class_id],
        )
        .await?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
}

/// Santri anggota kelas (unik).
pub async fn class_members(
    pool: &Pool,
    class_id: i64,
) -> Result<Vec<(i64, String, Option<String>)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT u.id, u.full_name, u.nis \
             FROM class_participants cp JOIN users u ON u.id = cp.user_id \
             WHERE cp.class_id = $1 AND u.role = 'santri' ORDER BY u.full_name",
            &[&class_id],
        )
        .await
        .context("class_members")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2)))
        .collect())
}

pub struct SchedRow {
    pub id: i64,
    pub title: String,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
    pub limit_time: chrono::NaiveTime,
    pub recurrence_type: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: Option<chrono::NaiveDate>,
}

/// Jadwal-jadwal milik kelas.
pub async fn class_schedules(pool: &Pool, class_id: i64) -> Result<Vec<SchedRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, COALESCE(title, ''), start_time, end_time, limit_entery_time, \
                    recurrence_type, start_date, end_date \
             FROM class_schedules WHERE class_id = $1 ORDER BY start_time",
            &[&class_id],
        )
        .await
        .context("class_schedules")?;
    Ok(rows
        .into_iter()
        .map(|r| SchedRow {
            id: r.get(0),
            title: r.get(1),
            start_time: r.get(2),
            end_time: r.get(3),
            limit_time: r.get(4),
            recurrence_type: r.get(5),
            start_date: r.get(6),
            end_date: r.get(7),
        })
        .collect())
}

/// Buat jadwal baru → id.
#[allow(clippy::too_many_arguments)]
pub async fn create_schedule(
    pool: &Pool,
    class_id: i64,
    title: &str,
    start_time: chrono::NaiveTime,
    end_time: chrono::NaiveTime,
    limit_time: chrono::NaiveTime,
    recurrence_type: &str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO class_schedules \
                (class_id, title, start_time, end_time, limit_entery_time, recurrence_type, start_date, end_date) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            &[&class_id, &title, &start_time, &end_time, &limit_time, &recurrence_type, &start_date, &end_date],
        )
        .await
        .context("create_schedule")?;
    Ok(row.get(0))
}

/// Buat sesi baru → id.
pub async fn create_session(
    pool: &Pool,
    class_id: i64,
    schedule_id: Option<i64>,
    teacher_id: Option<i64>,
    title: &str,
    session_date: chrono::NaiveDate,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO class_sessions (class_id, class_schedule_id, teacher_id, title, session_date) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[&class_id, &schedule_id, &teacher_id, &title, &session_date],
        )
        .await
        .context("create_session")?;
    Ok(row.get(0))
}

/// Tambah santri ke kelas (menempel ke sebuah jadwal — class_schedule_id NOT NULL).
/// Return true bila baru (bukan duplikat).
pub async fn add_member(
    pool: &Pool,
    class_id: i64,
    schedule_id: i64,
    user_id: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO class_participants (class_id, class_schedule_id, user_id) \
             VALUES ($1, $2, $3) ON CONFLICT (class_id, user_id, class_schedule_id) DO NOTHING",
            &[&class_id, &schedule_id, &user_id],
        )
        .await
        .context("add_member")?;
    Ok(n > 0)
}

/// Opsi pengajar (teacher/dewan_guru/supervisor) untuk dropdown buat sesi.
pub async fn teacher_options(pool: &Pool) -> Result<Vec<(i64, String)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, full_name FROM users \
             WHERE role IN ('teacher','dewan_guru','supervisor') AND is_active = TRUE \
             ORDER BY full_name",
            &[],
        )
        .await
        .context("teacher_options")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// Ubah jadwal (title/jam/recurrence/tanggal). Return true bila ada baris ter-update.
#[allow(clippy::too_many_arguments)]
pub async fn update_schedule(
    pool: &Pool,
    schedule_id: i64,
    title: &str,
    start_time: chrono::NaiveTime,
    end_time: chrono::NaiveTime,
    limit_time: chrono::NaiveTime,
    recurrence_type: &str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_schedules SET title = $2, start_time = $3, end_time = $4, \
                limit_entery_time = $5, recurrence_type = $6, start_date = $7, end_date = $8 \
             WHERE id = $1",
            &[
                &schedule_id,
                &title,
                &start_time,
                &end_time,
                &limit_time,
                &recurrence_type,
                &start_date,
                &end_date,
            ],
        )
        .await
        .context("update_schedule")?;
    Ok(n > 0)
}

/// Hapus jadwal. (class_sessions.class_schedule_id → SET? kolom nullable, ON DELETE
/// default NO ACTION → hapus manual referensi dulu agar aman.)
/// Hapus jadwal + sesi MENDATANG-nya (≥ `today`) yang belum dipakai (tak ada
/// absensi/chat) → tak meninggalkan sesi "yatim" yang membingungkan. Sesi lampau
/// atau yang sudah ada absensi/chat DILEPAS (class_schedule_id=NULL) agar histori
/// absensi tetap utuh. Semua dalam satu transaksi.
pub async fn delete_schedule(
    pool: &Pool,
    schedule_id: i64,
    today: chrono::NaiveDate,
) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("delete_schedule tx")?;

    // Hapus sesi mendatang yang AMAN (belum ada absensi & chat).
    tx.execute(
        "DELETE FROM class_sessions cs \
         WHERE cs.class_schedule_id = $1 AND cs.session_date >= $2 \
           AND NOT EXISTS (SELECT 1 FROM attendances a WHERE a.class_session_id = cs.id) \
           AND NOT EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id = cs.id)",
        &[&schedule_id, &today],
    )
    .await
    .context("delete_schedule hapus sesi mendatang")?;

    // Sisanya (lampau / sudah dipakai) → dilepas, histori tetap ada.
    tx.execute(
        "UPDATE class_sessions SET class_schedule_id = NULL WHERE class_schedule_id = $1",
        &[&schedule_id],
    )
    .await
    .context("delete_schedule lepas sesi")?;

    let n = tx
        .execute("DELETE FROM class_schedules WHERE id = $1", &[&schedule_id])
        .await
        .context("delete_schedule")?;
    tx.commit().await.context("delete_schedule commit")?;
    Ok(n > 0)
}

/// Hapus sesi MENDATANG (≥ `today`) milik jadwal ini yang tanggalnya TIDAK ada di
/// `valid` (tanggal-tanggal yang sah menurut jadwal terbaru) DAN belum dipakai
/// (tanpa absensi/chat). Dipakai setelah update jadwal agar sesi mendatang
/// mengikuti rentang/pola baru; sesi dalam rentang (mis. sudah diberi pengajar /
/// ditandai libur) dibiarkan. Return jumlah sesi terhapus.
pub async fn delete_future_sessions_not_in(
    pool: &Pool,
    schedule_id: i64,
    today: chrono::NaiveDate,
    valid: &[chrono::NaiveDate],
) -> Result<u64> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "DELETE FROM class_sessions cs \
             WHERE cs.class_schedule_id = $1 AND cs.session_date >= $2 \
               AND NOT (cs.session_date = ANY($3::date[])) \
               AND NOT EXISTS (SELECT 1 FROM attendances a WHERE a.class_session_id = cs.id) \
               AND NOT EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id = cs.id)",
            &[&schedule_id, &today, &valid],
        )
        .await
        .context("delete_future_sessions_not_in")?;
    Ok(n)
}

/// Info sebuah jadwal (untuk generate sesi): (class_id, start_time, recurrence, start_date).
pub async fn schedule_info(
    pool: &Pool,
    schedule_id: i64,
) -> Result<Option<(i64, String, String, chrono::NaiveDate)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT class_id, COALESCE(title, ''), recurrence_type, start_date \
             FROM class_schedules WHERE id = $1",
            &[&schedule_id],
        )
        .await?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))))
}

/// Insert BANYAK sesi sekaligus (generate bulanan/mendatang) dalam SATU query
/// set-based (`unnest` + `NOT EXISTS`) — cepat & idempotent, tak menggandakan
/// (schedule, tanggal) yang sudah ada. Return jumlah sesi baru.
pub async fn insert_sessions(
    pool: &Pool,
    class_id: i64,
    schedule_id: i64,
    title: &str,
    dates: &[chrono::NaiveDate],
) -> Result<i64> {
    if dates.is_empty() {
        return Ok(0);
    }
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO class_sessions (class_id, class_schedule_id, title, session_date) \
             SELECT $1, $2, $3, d FROM unnest($4::date[]) AS d \
             WHERE NOT EXISTS ( \
                SELECT 1 FROM class_sessions cs \
                WHERE cs.class_schedule_id = $2 AND cs.session_date = d \
             )",
            &[&class_id, &schedule_id, &title, &dates],
        )
        .await
        .context("insert_sessions")?;
    Ok(n as i64)
}

/// Keluarkan santri dari kelas (semua barisnya lintas-jadwal).
pub async fn remove_member(pool: &Pool, class_id: i64, user_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "DELETE FROM class_participants WHERE class_id = $1 AND user_id = $2",
            &[&class_id, &user_id],
        )
        .await
        .context("remove_member")?;
    Ok(n > 0)
}

/// Set/ubah pengajar sebuah sesi (teacher_id NULL bila 0/None).
pub async fn set_session_teacher(
    pool: &Pool,
    session_id: i64,
    teacher_id: Option<i64>,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_sessions SET teacher_id = $2 WHERE id = $1",
            &[&session_id, &teacher_id],
        )
        .await
        .context("set_session_teacher")?;
    Ok(n > 0)
}

/// Set status sesi (mis. 'cancelled' = libur, 'scheduled' = aktif kembali).
pub async fn set_session_status(pool: &Pool, session_id: i64, status: &str) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_sessions SET status = $2 WHERE id = $1",
            &[&session_id, &status],
        )
        .await
        .context("set_session_status")?;
    Ok(n > 0)
}

/// Beberapa santri aktif (untuk daftar default form Tambah Santri, tanpa cari).
pub async fn some_students(pool: &Pool, limit: i64) -> Result<Vec<super::parents::StudentRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, cl.name \
             FROM users u \
             LEFT JOIN class_participants cp ON cp.user_id = u.id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE u.role = 'santri' AND u.is_active = TRUE \
             ORDER BY u.full_name LIMIT $1",
            &[&limit],
        )
        .await
        .context("some_students")?;
    Ok(rows
        .into_iter()
        .map(|r| super::parents::StudentRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
        })
        .collect())
}

/// Jadwal aktif sebuah kelas untuk auto-generate sesi mendatang.
// Tuple jadwal aktif untuk materialisasi: (..., start_date, end_date). end_date
// WAJIB dibawa agar materialisasi TIDAK melewati akhir jadwal.
type ActiveSched = (
    i64,
    String,
    String,
    chrono::NaiveDate,
    Option<chrono::NaiveDate>,
);

pub async fn active_schedules_of(pool: &Pool, class_id: i64) -> Result<Vec<ActiveSched>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, COALESCE(title, ''), recurrence_type, start_date, end_date \
             FROM class_schedules WHERE class_id = $1 AND status = 'active'",
            &[&class_id],
        )
        .await
        .context("active_schedules_of")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
        .collect())
}

/// Semua jadwal aktif LINTAS-kelas (class_id, id, title, recurrence, start_date,
/// end_date) — untuk materialisasi sesi di task background (bukan per-request).
pub async fn active_schedules_all(
    pool: &Pool,
) -> Result<
    Vec<(
        i64,
        i64,
        String,
        String,
        chrono::NaiveDate,
        Option<chrono::NaiveDate>,
    )>,
> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT class_id, id, COALESCE(title, ''), recurrence_type, start_date, end_date \
             FROM class_schedules WHERE status = 'active'",
            &[],
        )
        .await
        .context("active_schedules_all")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4), r.get(5)))
        .collect())
}

/// Update kolom rekaman sesi (dipanggil tiap chunk siaran — best effort).
pub async fn update_recording(
    pool: &Pool,
    session_id: i64,
    path: &str,
    mime: &str,
    size: i64,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE class_sessions SET recording_path = $2, recording_mime_type = $3, \
         recording_size = $4 WHERE id = $1",
        &[&session_id, &path, &mime, &size],
    )
    .await
    .context("update_recording")?;
    Ok(())
}
