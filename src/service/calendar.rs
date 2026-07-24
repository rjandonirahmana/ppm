//! service/calendar.rs — Kalender akademik bulanan. Sesi kelas pada bulan
//! tertentu, di-scope peran:
//!   • admin/supervisor(pamong)/teacher(guru)/dewan_guru → SEMUA kelas
//!   • parent → kelas anak-anak terhubung
//!   • santri → kelas yang diikuti sendiri
//! Grid (leading_blanks Senin-first + days_in_month) dihitung di sini agar UI
//! cukup me-render tanpa aritmetika tanggal.

use anyhow::{bail, Result};
use chrono::{Datelike, NaiveDate};
use deadpool_postgres::Pool;

use super::fmt::{wib, BULAN_PANJANG};
use crate::models::{CalendarData, CalendarItem, SessionUser};
use crate::repository as repo;

fn status_display(status: &str) -> (&'static str, &'static str) {
    match status {
        "ongoing" => ("Berlangsung", "ongoing"),
        "finished" => ("Selesai", "finished"),
        "cancelled" => ("Libur", "cancelled"),
        _ => ("Terjadwal", "scheduled"),
    }
}

/// (hari pertama, hari terakhir, jumlah hari) bulan `month` tahun `year`.
fn month_bounds(year: i32, month: u32) -> Option<(NaiveDate, NaiveDate, u32)> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let next_first = NaiveDate::from_ymd_opt(ny, nm, 1)?;
    let last = next_first.pred_opt()?;
    Some((first, last, last.day()))
}

pub async fn calendar_data(
    pool: &Pool,
    user: &SessionUser,
    year: i32,
    month: u32,
) -> Result<CalendarData> {
    // Sentinel year==0 → bulan berjalan (klien tak perlu tahu tanggal saat
    // buka halaman; navigasi berikutnya pakai prev/next dari respons).
    let (year, month) = if year == 0 { current_month() } else { (year, month) };
    if !(1..=12).contains(&month) || !(2000..=2100).contains(&year) {
        bail!("Bulan/tahun tidak valid.");
    }
    let Some((first, last, days_in_month)) = month_bounds(year, month) else {
        bail!("Tanggal tidak valid.");
    };

    // Sesi bulan ini, di-scope peran. limit tinggi (semua sesi 1 bulan muat).
    let (rows, scope_label) = match user.role.as_str() {
        "admin" | "supervisor" | "teacher" | "dewan_guru" => {
            (repo::all_sessions(pool, first, last, 2000).await?, "Semua kelas")
        }
        "parent" => (
            repo::sessions_for_parent(pool, user.id, first, last, 2000).await?,
            "Kelas anak Anda",
        ),
        _ => (
            repo::sessions_for_student(pool, user.id, first, last, 2000).await?,
            "Kelas Anda",
        ),
    };

    let items: Vec<CalendarItem> = rows
        .into_iter()
        .map(|r| {
            let (status_label, status_kind) = status_display(&r.status);
            CalendarItem {
                day: r.session_date.day(),
                session_id: r.id,
                time_label: r
                    .start_time
                    .map(|t| t.format("%H:%M").to_string())
                    .unwrap_or_else(|| "-".into()),
                title: r.title.unwrap_or_else(|| r.class_name.clone()),
                class_name: r.class_name,
                teacher: r.teacher.unwrap_or_else(|| "-".into()),
                category: r.category.filter(|c| !c.is_empty()).unwrap_or_else(|| "-".into()),
                status_kind: status_kind.into(),
                status_label: status_label.into(),
            }
        })
        .collect();

    // Grid Senin-first: jumlah sel kosong sebelum tanggal 1.
    let leading_blanks = first.weekday().num_days_from_monday();
    let today = chrono::Utc::now().with_timezone(&wib()).date_naive();
    let today_day = if today.year() == year && today.month() == month { today.day() } else { 0 };

    let (prev_year, prev_month) = if month == 1 { (year - 1, 12) } else { (year, month - 1) };
    let (next_year, next_month) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };

    Ok(CalendarData {
        year,
        month,
        month_label: format!("{} {}", BULAN_PANJANG[(month - 1) as usize], year),
        prev_year,
        prev_month,
        next_year,
        next_month,
        leading_blanks,
        days_in_month,
        today_day,
        scope_label: scope_label.into(),
        items,
    })
}

/// Bulan berjalan (WIB) sebagai (year, month) — default saat halaman dibuka.
pub fn current_month() -> (i32, u32) {
    let now = chrono::Utc::now().with_timezone(&wib()).date_naive();
    (now.year(), now.month())
}
