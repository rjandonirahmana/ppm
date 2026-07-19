//! service/sessions.rs — Daftar sesi kelas per-peran.
//! Santri → hanya sesi kelas yang diikutinya; admin/pamong/dewan guru → SEMUA
//! (nantinya bisa mengelola/update sesi dari halaman yang sama).

use anyhow::Result;
use deadpool_postgres::Pool;

use chrono::{Duration, Utc};

use super::fmt::{fmt_date, wib};
use crate::models::{SessionItem, SessionsData, SessionUser};
use crate::repository as repo;

fn status_display(status: &str) -> (&'static str, &'static str) {
    match status {
        "ongoing" => ("Berlangsung", "ongoing"),
        "finished" => ("Selesai", "finished"),
        "cancelled" => ("Dibatalkan", "cancelled"),
        _ => ("Terjadwal", "scheduled"),
    }
}

pub async fn list_for(pool: &Pool, user: &SessionUser) -> Result<SessionsData> {
    // Hanya sesi 1 MINGGU TERAKHIR yang sudah lewat (s/d hari ini WIB).
    let until = Utc::now().with_timezone(&wib()).date_naive();
    let since = until - Duration::days(7);
    let all_scope = matches!(user.role.as_str(), "admin" | "supervisor" | "dewan_guru");
    let rows = if all_scope {
        repo::all_sessions(pool, since, until, 100).await?
    } else if user.role == "teacher" {
        repo::sessions_for_teacher(pool, user.id, since, until, 100).await?
    } else {
        repo::sessions_for_student(pool, user.id, since, until, 100).await?
    };

    let items = rows
        .into_iter()
        .map(|r| {
            let (status_label, status_kind) = status_display(&r.status);
            SessionItem {
                id: r.id,
                title: r.title.unwrap_or_else(|| r.class_name.clone()),
                class_name: r.class_name,
                date_label: fmt_date(r.session_date),
                time_label: r
                    .start_time
                    .map(|t| format!("{} WIB", t.format("%H:%M")))
                    .unwrap_or_else(|| "-".into()),
                status_label: status_label.into(),
                status_kind: status_kind.into(),
                teacher: r.teacher.unwrap_or_else(|| "-".into()),
                teacher_id: r.teacher_id,
            }
        })
        .collect();

    Ok(SessionsData {
        role: user.role.clone(),
        all_scope,
        items,
    })
}
