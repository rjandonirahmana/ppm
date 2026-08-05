//! service/dashboard.rs — Perakitan payload dashboard (query paralel + format).

use anyhow::Result;
use chrono::Datelike;
use deadpool_postgres::Pool;

use super::fmt::{fmt_schedule, fmt_when, wib};
use crate::models::{
    AnalisisData, AttendanceItem, ClassRank, LatestAtt, LiveSesi, PoinData, PointRow, SantriHome,
    ScheduleInfo, StafHome, TeacherInsight, TrendPoint,
};
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
        bail_user!("unauth");
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

/// Inisial 1-2 huruf dari nama (avatar bulat).
fn initial_of(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_uppercase()
}

/// Mapping baris sesi hari ini → LiveSesi (dipakai staf/pamong/dewan).
pub(crate) fn map_live(rows: Vec<crate::repository::LiveSesiRow>) -> Vec<LiveSesi> {
    rows.into_iter()
        .map(|s| LiveSesi {
            id: s.id,
            title: s.title,
            teacher: s.teacher,
            santri_count: s.santri_count,
            // `status` di DB tak pernah bergerak sendiri ke 'ongoing' — sesi
            // yang jamnya sedang berjalan tetap tertulis 'scheduled'. Maka jam
            // WIB ikut menentukan: sesi terjadwal yang sekarang berada di
            // antara jam mulai & selesai ditampilkan sebagai SEDANG
            // BERLANGSUNG. Tanpa ini kartu beranda menyebut sesi yang sedang
            // berjalan sebagai "jadwal berikutnya".
            state: match s.state.as_str() {
                "ongoing" => "live".into(),
                "scheduled" if s.ongoing => "live".into(),
                "scheduled" => "upcoming".into(),
                _ => "break".into(),
            },
            time_label: s
                .time_label
                .map(|t| format!("{} WIB", t.format("%H:%M")))
                .unwrap_or_else(|| "-".into()),
            past: s.past,
        })
        .collect()
}

/// Mapping kehadiran terbaru → LatestAtt (dipakai staf/pamong).
pub(crate) fn map_latest(rows: Vec<crate::repository::LatestAttRow>) -> Vec<LatestAtt> {
    rows.into_iter()
        .map(|a| {
            let (status_label, kind) = super::santri::status_display(&a.status);
            LatestAtt {
                name: a.name.clone(),
                initial: initial_of(&a.name),
                class_name: a.class_name.unwrap_or_else(|| "-".into()),
                time_label: format!("{} WIB", a.scanned_at.with_timezone(&wib()).format("%H:%M")),
                status_label: status_label.into(),
                kind: kind.into(),
            }
        })
        .collect()
}

/// Dashboard staf/admin (/staf): statistik hari ini + sesi live + kehadiran terbaru.
pub async fn staf_home(pool: &Pool, name: &str) -> Result<StafHome> {
    let (stats, live, latest) = tokio::join!(
        repo::staf_stats(pool),
        repo::today_sessions(pool, 6),
        repo::latest_attendance(pool, 8),
    );
    let (total_santri, santri_growth_month, hadir_today, izin_pending) = stats?;
    let pct = if total_santri > 0 { ((hadir_today * 100) / total_santri) as i32 } else { 0 };

    let live = map_live(live?);
    let latest = map_latest(latest?);

    Ok(StafHome {
        name: name.to_string(),
        total_santri,
        santri_growth_month,
        hadir_today,
        pct,
        izin_pending,
        live,
        latest,
    })
}

const HARI_PENDEK: [&str; 7] = ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"];

/// Dashboard analisis guru (/guru, cakupan kelas sendiri) atau dewan guru
/// (/dewan-guru, `teacher_id = None` → seluruh pesantren).
pub async fn analisis(pool: &Pool, name: &str, teacher_id: Option<i64>) -> Result<AnalisisData> {
    let (summary, trend, ranking, insight, today) = tokio::join!(
        repo::analisis_summary(pool, teacher_id),
        repo::attendance_trend_7d(pool, teacher_id),
        repo::class_ranking(pool, teacher_id, 5),
        async {
            if teacher_id.is_none() {
                repo::teacher_insight(pool, 5).await
            } else {
                Ok(vec![])
            }
        },
        repo::today_sessions(pool, 6),
    );
    let (attendance_pct, avg_points, sessions_verified) = summary?;
    let today = map_live(today?);

    let trend = trend?
        .into_iter()
        .map(|(d, pct)| TrendPoint { label: HARI_PENDEK[d.weekday().num_days_from_sunday() as usize].into(), pct })
        .collect();

    let class_ranking = ranking?
        .into_iter()
        .map(|r| ClassRank {
            name: r.name,
            attendance_pct: r.attendance_pct,
            avg_points: r.avg_points,
            santri_count: r.santri_count,
        })
        .collect();

    let teacher_insight = insight?
        .into_iter()
        .map(|t| TeacherInsight { name: t.name, sessions_count: t.sessions_count, attendance_pct: t.attendance_pct })
        .collect();

    Ok(AnalisisData {
        name: name.to_string(),
        is_dewan: teacher_id.is_none(),
        attendance_pct,
        avg_points,
        sessions_verified,
        trend,
        class_ranking,
        teacher_insight,
        today,
    })
}

/// Papan poin santri (/poin staf/pamong — cakupan sendiri; /poin-dewan dewan
/// guru/admin — `can_adjust=true`, boleh tambah/kurangi poin manual).
pub async fn poin_data(pool: &Pool, teacher_id: Option<i64>, can_adjust: bool) -> Result<PoinData> {
    let (avg, board) = tokio::join!(repo::points_avg(pool, teacher_id), repo::points_board(pool, teacher_id, 20, true));
    let (avg_points, total_santri) = avg?;
    let top = board?
        .into_iter()
        .map(|r| PointRow {
            user_id: r.user_id,
            name: r.name.clone(),
            nis: r.nis,
            class_name: r.class_name,
            points: r.points,
            initial: initial_of(&r.name),
        })
        .collect();
    Ok(PoinData { can_adjust, avg_points, total_santri, top })
}
