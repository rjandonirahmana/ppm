//! repository/schedule.rs — Query class_schedules & class_sessions.

use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveTime};
use deadpool_postgres::Pool;

pub struct ScheduleRow {
    pub title: Option<String>,
    pub class_name: String,
    pub start_time: NaiveTime,
}

/// Jadwal aktif terdekat milik santri (MVP: urut jam mulai).
pub async fn next_schedule(pool: &Pool, user_id: i64) -> Result<Option<ScheduleRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cs.title, c.name, cs.start_time \
             FROM class_participants cp \
             JOIN class_schedules cs ON cs.id = cp.class_schedule_id AND cs.status = 'active' \
             JOIN classes c ON c.id = cs.class_id \
             WHERE cp.user_id = $1 \
               AND cs.start_date <= CURRENT_DATE \
               AND (cs.end_date IS NULL OR cs.end_date >= CURRENT_DATE) \
             ORDER BY cs.start_time ASC LIMIT 1",
            &[&user_id],
        )
        .await
        .context("next_schedule")?;
    Ok(row.map(|r| ScheduleRow {
        title: r.get(0),
        class_name: r.get(1),
        start_time: r.get(2),
    }))
}

pub struct ActiveSchedule {
    pub id: i64,
    pub limit_entry: NaiveTime,
}

/// Jadwal aktif yang sedang berlangsung untuk user pada waktu WIB tertentu.
/// Jendela masuk: 45 menit sebelum start_time s/d end_time.
pub async fn active_schedule_now(
    pool: &Pool,
    user_id: i64,
    today: NaiveDate,
    now_time: NaiveTime,
) -> Result<Option<ActiveSchedule>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cs.id, cs.limit_entery_time \
             FROM class_participants cp \
             JOIN class_schedules cs ON cs.id = cp.class_schedule_id AND cs.status = 'active' \
             WHERE cp.user_id = $1 \
               AND cs.start_date <= $2 AND (cs.end_date IS NULL OR cs.end_date >= $2) \
               AND $3::time >= cs.start_time - INTERVAL '45 minutes' \
               AND $3::time <= cs.end_time \
             ORDER BY cs.start_time LIMIT 1",
            &[&user_id, &today, &now_time],
        )
        .await
        .context("active_schedule_now")?;
    Ok(row.map(|r| ActiveSchedule {
        id: r.get(0),
        limit_entry: r.get(1),
    }))
}

/// Sesi kelas hari ini untuk jadwal tsb (bila guru sudah memulai sesi).
pub async fn session_for_schedule_today(
    pool: &Pool,
    schedule_id: i64,
    today: NaiveDate,
) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id FROM class_sessions \
             WHERE class_schedule_id = $1 AND session_date = $2 LIMIT 1",
            &[&schedule_id, &today],
        )
        .await?;
    Ok(row.map(|r| r.get(0)))
}

pub struct SessionRow {
    pub id: i64,
    pub title: Option<String>,
    pub class_name: String,
    pub session_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub status: String,
    pub teacher: Option<String>,
}

const SESSION_COLS: &str = "SELECT s.id, COALESCE(s.title, cs.title), c.name, s.session_date, \
     cs.start_time, s.status, t.full_name \
     FROM class_sessions s \
     JOIN classes c ON c.id = s.class_id \
     LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
     LEFT JOIN users t ON t.id = s.teacher_id";

fn row_to_session(r: tokio_postgres::Row) -> SessionRow {
    SessionRow {
        id: r.get(0),
        title: r.get(1),
        class_name: r.get(2),
        session_date: r.get(3),
        start_time: r.get(4),
        status: r.get(5),
        teacher: r.get(6),
    }
}

/// SEMUA sesi (admin/pamong/dewan guru) — terbaru dulu.
pub async fn all_sessions(pool: &Pool, limit: i64) -> Result<Vec<SessionRow>> {
    let c = pool.get().await?;
    let sql = format!("{SESSION_COLS} ORDER BY s.session_date DESC, s.id DESC LIMIT $1");
    let rows = c.query(&sql, &[&limit]).await.context("all_sessions")?;
    Ok(rows.into_iter().map(row_to_session).collect())
}

/// Sesi kelas-kelas yang DIIKUTI santri ini saja.
pub async fn sessions_for_student(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<SessionRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "{SESSION_COLS} \
         WHERE s.class_id IN (SELECT class_id FROM class_participants WHERE user_id = $1) \
         ORDER BY s.session_date DESC, s.id DESC LIMIT $2"
    );
    let rows = c.query(&sql, &[&user_id, &limit]).await.context("sessions_for_student")?;
    Ok(rows.into_iter().map(row_to_session).collect())
}
