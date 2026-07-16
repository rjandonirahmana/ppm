//! service/fmt.rs — Format waktu Indonesia (WIB) untuk tampilan.

use chrono::{DateTime, Datelike, FixedOffset, NaiveTime, Utc};

pub fn wib() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("WIB offset")
}

/// "Hari ini, 15:45 WIB" / "Kemarin, 04:30 WIB" / "20 Okt 2025, 19:30 WIB"
pub fn fmt_when(ts: DateTime<Utc>) -> String {
    const BULAN: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
    ];
    let now = Utc::now().with_timezone(&wib());
    let t = ts.with_timezone(&wib());
    let day = if t.date_naive() == now.date_naive() {
        "Hari ini".to_string()
    } else if Some(t.date_naive()) == now.date_naive().pred_opt() {
        "Kemarin".to_string()
    } else {
        format!(
            "{} {} {}",
            t.date_naive().day(),
            BULAN[t.date_naive().month0() as usize],
            t.date_naive().year()
        )
    };
    format!("{day}, {} WIB", t.format("%H:%M"))
}

/// Label jadwal: "Hari ini, 04:30 WIB" atau "Besok, 04:30 WIB".
pub fn fmt_schedule(start: NaiveTime) -> String {
    let now = Utc::now().with_timezone(&wib());
    let day = if now.time() < start { "Hari ini" } else { "Besok" };
    format!("{day}, {} WIB", start.format("%H:%M"))
}

pub const BULAN_PENDEK: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "Mei", "Jun", "Jul", "Agu", "Sep", "Okt", "Nov", "Des",
];
pub const BULAN_PANJANG: [&str; 12] = [
    "Januari", "Februari", "Maret", "April", "Mei", "Juni", "Juli", "Agustus", "September",
    "Oktober", "November", "Desember",
];

/// Tanggal+jam eksplisit: "21 Okt 2025, 20:00 WIB" (untuk daftar riwayat).
pub fn fmt_dt_full(ts: DateTime<Utc>) -> String {
    let t = ts.with_timezone(&wib());
    let d = t.date_naive();
    format!(
        "{} {} {}, {} WIB",
        d.day(),
        BULAN_PENDEK[d.month0() as usize],
        d.year(),
        t.format("%H:%M")
    )
}

/// Label grup bulan: "September 2025".
pub fn fmt_month(ts: DateTime<Utc>) -> String {
    let d = ts.with_timezone(&wib()).date_naive();
    format!("{} {}", BULAN_PANJANG[d.month0() as usize], d.year())
}

/// Rentang tanggal izin: "12 – 13 Nov 2025" / "12 Nov 2025".
pub fn fmt_range(start: chrono::NaiveDate, end: Option<chrono::NaiveDate>) -> String {
    let f = |d: chrono::NaiveDate| {
        format!("{} {} {}", d.day(), BULAN_PENDEK[d.month0() as usize], d.year())
    };
    match end {
        Some(e) if e != start => format!("{} – {}", f(start), f(e)),
        _ => f(start),
    }
}

/// Label relatif: "Baru saja" / "5 menit lalu" / "2 jam lalu" / "3 hari lalu".
pub fn fmt_ago(ts: DateTime<Utc>) -> String {
    let secs = (Utc::now() - ts).num_seconds().max(0);
    match secs {
        0..=59 => "Baru saja".into(),
        60..=3599 => format!("{} menit lalu", secs / 60),
        3600..=86_399 => format!("{} jam lalu", secs / 3600),
        _ => format!("{} hari lalu", secs / 86_400),
    }
}

/// Label tanggal: "Hari ini" / "Besok" / "Kemarin" / "16 Jul 2026" (WIB).
pub fn fmt_date(d: chrono::NaiveDate) -> String {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    if d == today {
        "Hari ini".into()
    } else if Some(d) == today.succ_opt() {
        "Besok".into()
    } else if Some(d) == today.pred_opt() {
        "Kemarin".into()
    } else {
        format!("{} {} {}", d.day(), BULAN_PENDEK[d.month0() as usize], d.year())
    }
}
