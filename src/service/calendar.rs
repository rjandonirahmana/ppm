//! service/calendar.rs — Kalender akademik bulanan. Sesi kelas pada bulan
//! tertentu, di-scope peran:
//!   • admin/supervisor(pamong)/teacher(guru)/dewan_guru → SEMUA kelas
//!   • parent → kelas anak-anak terhubung
//!   • santri → kelas yang diikuti sendiri
//! Grid (leading_blanks Senin-first + days_in_month) dihitung di sini agar UI
//! cukup me-render tanpa aritmetika tanggal.

use anyhow::Result;
use chrono::{Datelike, NaiveDate, NaiveTime};
use deadpool_postgres::Pool;

use super::fmt::{fmt_date, wib, BULAN_PANJANG};
use crate::config::WahaConfig;
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
        bail_user!("Bulan/tahun tidak valid.");
    }
    let Some((first, last, days_in_month)) = month_bounds(year, month) else {
        bail_user!("Tanggal tidak valid.");
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

    let active_semester = super::semester::active_label(pool).await;

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
        active_semester,
        items,
    })
}

/// Bulan berjalan (WIB) sebagai (year, month) — default saat halaman dibuka.
pub fn current_month() -> (i32, u32) {
    let now = chrono::Utc::now().with_timezone(&wib()).date_naive();
    (now.year(), now.month())
}

// ── Kirim jadwal sesi ke Google Calendar santri (via WhatsApp) ───────────────
// Punya email/HP santri TIDAK cukup untuk MENULIS langsung ke kalender orang
// (Google memblokir by design). Pendekatan yang dipakai: kirim tautan
// "Tambah ke Google Calendar" (action=TEMPLATE) lewat WhatsApp — santri tap 1×
// di HP → Google Calendar terbuka terisi → Simpan. Zero OAuth, reuse WAHA.

/// Percent-encode nilai query URL (RFC 3986 unreserved dibiarkan apa adanya).
fn enc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// Tautan "Tambah ke Google Calendar" untuk satu sesi. Waktu dikirim sebagai
/// waktu lokal + `ctz=Asia/Jakarta` (tanpa konversi UTC). Sesi tanpa jam
/// (ad-hoc) → acara sepanjang hari. `end` diabaikan bila ≤ `start` (default +1 jam).
pub fn google_calendar_link(
    title: &str,
    class_name: &str,
    teacher: &str,
    date: NaiveDate,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
) -> String {
    let dates = match start {
        Some(s) => {
            let start_dt = date.and_time(s);
            let end_dt = match end {
                Some(e) if e > s => date.and_time(e),
                _ => start_dt + chrono::Duration::hours(1),
            };
            format!(
                "{}/{}",
                start_dt.format("%Y%m%dT%H%M%S"),
                end_dt.format("%Y%m%dT%H%M%S")
            )
        }
        // Acara sepanjang hari: tanggal akhir eksklusif (hari berikutnya).
        None => {
            let next = date.succ_opt().unwrap_or(date);
            format!("{}/{}", date.format("%Y%m%d"), next.format("%Y%m%d"))
        }
    };
    let details = format!("Kelas: {class_name}\nPengajar: {teacher}\n\nJadwal dari AFM SMART.");
    format!(
        "https://calendar.google.com/calendar/render?action=TEMPLATE\
         &text={}&dates={}&details={}&ctz=Asia%2FJakarta",
        enc(title),
        dates,
        enc(&details),
    )
}

/// Broadcast jadwal sebuah sesi ke WhatsApp seluruh santri kelasnya, berisi
/// tautan "Tambah ke Google Calendar". Staf-only (guard di server-fn).
/// Return (terkirim, dilewati_tanpa_HP). Kegagalan kirim per-nomor dicatat log,
/// tak menggagalkan keseluruhan.
pub async fn send_schedule_wa(
    pool: &Pool,
    http: &reqwest::Client,
    waha: &WahaConfig,
    session_id: i64,
) -> Result<(i64, i64)> {
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        bail_user!("Sesi tidak ditemukan.");
    };
    let title = d.title.clone().unwrap_or_else(|| d.class_name.clone());
    let teacher = d.teacher.clone().unwrap_or_else(|| "Belum ditentukan".into());
    let link = google_calendar_link(
        &title,
        &d.class_name,
        &teacher,
        d.session_date,
        d.start_time,
        d.end_time,
    );

    let time_label = d
        .start_time
        .map(|t| format!("{} WIB", t.format("%H:%M")))
        .unwrap_or_else(|| "Menyusul".into());

    let contacts = repo::class_student_contacts(pool, d.class_id).await?;
    let (mut sent, mut skipped) = (0i64, 0i64);
    for (name, phone) in contacts {
        let Some(phone) = phone.filter(|p| !p.is_empty()) else {
            skipped += 1;
            continue;
        };
        let text = format!(
            "Assalamu'alaikum {name} 🕌\n\n\
             Jadwal sesi *{title}* kelas *{}*:\n\
             🗓️ {}\n\
             ⏰ {time_label}\n\
             👤 Pengajar: {teacher}\n\n\
             Tambahkan ke Google Calendar (tap):\n{link}",
            d.class_name,
            fmt_date(d.session_date),
        );
        match super::registration::send_wa_text(http, waha, &phone, &text).await {
            Ok(_) => sent += 1,
            Err(e) => tracing::warn!(session = session_id, "WA jadwal gagal ke {phone}: {e}"),
        }
    }
    tracing::info!(session_id, sent, skipped, "broadcast jadwal WA");
    Ok((sent, skipped))
}

#[cfg(test)]
mod tests {
    use super::google_calendar_link;
    use chrono::{NaiveDate, NaiveTime};

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 8, 1).unwrap()
    }

    #[test]
    fn tautan_bersesi_waktu_lokal_wib() {
        let link = google_calendar_link(
            "Tahfidz Pagi",
            "Kelas 2A",
            "Ust. Ali",
            d(),
            NaiveTime::from_hms_opt(4, 40, 0),
            NaiveTime::from_hms_opt(5, 40, 0),
        );
        assert!(link.contains("dates=20260801T044000/20260801T054000"));
        assert!(link.contains("ctz=Asia%2FJakarta"));
        // spasi ter-encode, apostrof aman
        assert!(link.contains("text=Tahfidz%20Pagi"));
    }

    #[test]
    fn tanpa_jam_jadi_acara_sepanjang_hari() {
        let link = google_calendar_link("Rapat", "2A", "-", d(), None, None);
        assert!(link.contains("dates=20260801/20260802"));
    }

    #[test]
    fn end_tak_valid_default_satu_jam() {
        let link = google_calendar_link(
            "X",
            "2A",
            "-",
            d(),
            NaiveTime::from_hms_opt(23, 30, 0),
            NaiveTime::from_hms_opt(22, 0, 0), // ≤ start → diabaikan
        );
        // +1 jam melewati tengah malam → tanggal akhir naik.
        assert!(link.contains("dates=20260801T233000/20260802T003000"));
    }
}
