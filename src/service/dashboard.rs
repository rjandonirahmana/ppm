//! service/dashboard.rs — Perakitan payload dashboard (query paralel + format).

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use super::fmt::{fmt_schedule, fmt_when};
use crate::models::{AttendanceItem, SantriHome, ScheduleInfo};
use crate::repository as repo;

/// Payload dashboard santri. Empat query dijalankan PARALEL (satu round-trip
/// latensi, pola sama e-ticketing futures::join!).
pub async fn santri_home(pool: &Pool, user_id: i64) -> Result<SantriHome> {
    let (home, recent, schedule, progress, month_points) = tokio::join!(
        repo::user_home(pool, user_id),
        repo::recent_attendances(pool, user_id, 3),
        repo::next_schedule(pool, user_id),
        repo::month_progress(pool, user_id),
        repo::month_points(pool, user_id),
    );

    let Some(home) = home? else {
        bail!("unauth");
    };

    let recent = recent?
        .into_iter()
        .map(|a| {
            let gate = a.gate_label.unwrap_or_else(|| "Gate".into());
            let (title, badge, kind) = match a.status.as_str() {
                "present" => (
                    format!("Hadir - {gate}"),
                    if a.verify_status == "pending" { "Menunggu" } else { "Tepat Waktu" },
                    "present",
                ),
                "late" => (format!("Terlambat - {gate}"), "Terlambat", "late"),
                "permit" => ("Izin".to_string(), "Disetujui", "permit"),
                "sick" => ("Izin - Sakit".to_string(), "Disetujui", "sick"),
                _ => ("Tidak Hadir".to_string(), "Alpha", "absent"),
            };
            AttendanceItem {
                title,
                sub: fmt_when(a.scanned_at),
                badge: badge.to_string(),
                kind: kind.to_string(),
            }
        })
        .collect();

    let schedule = schedule?.map(|s| ScheduleInfo {
        title: s.title.unwrap_or_else(|| s.class_name.clone()),
        class_name: s.class_name,
        time_label: fmt_schedule(s.start_time),
    });

    let (hadir, total) = progress?;
    let month_pct = (total > 0).then(|| ((hadir * 100) / total) as i32);

    Ok(SantriHome {
        name: home.full_name,
        points: home.points,
        schedule,
        recent,
        month_pct,
        month_points: month_points.unwrap_or(0),
    })
}
