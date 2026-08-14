//! service/fmt.rs — Format waktu Indonesia (WIB) untuk tampilan.

use chrono::{DateTime, Datelike, FixedOffset, NaiveTime, TimeZone, Utc};

pub fn wib() -> FixedOffset {
    FixedOffset::east_opt(7 * 3600).expect("WIB offset")
}

/// Tanggal hari ini menurut WIB.
///
/// Server berjalan pada UTC, dan `Utc::now().date_naive()` sudah berganti hari
/// sejak pukul 07:00 WIB — cukup untuk membuat "sedang berjalan" salah selama
/// tujuh jam tiap hari. Semua pertanyaan "apakah ini masih berlaku hari ini?"
/// memakai fungsi ini.
pub fn today_wib() -> chrono::NaiveDate {
    Utc::now().with_timezone(&wib()).date_naive()
}

/// Awal hari WIB `n` hari yang lalu, dalam UTC — untuk membatasi query "N hari
/// terakhir".
///
/// Sengaja memakai batas HARI KALENDER, bukan "N × 24 jam" dari sekarang:
/// santri yang membuka rapor pukul 06:00 dan pukul 22:00 di hari yang sama
/// harus melihat rentang yang sama. `days_ago_wib(2)` = awal kemarin-lusa,
/// yaitu 3 hari kalender bersama hari ini.
pub fn days_ago_wib(n: i64) -> DateTime<Utc> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let start = today - chrono::Duration::days(n);
    wib()
        .from_local_datetime(&start.and_hms_opt(0, 0, 0).expect("00:00 valid"))
        .single()
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(Utc::now)
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

/// Tanggal lengkap berbahasa Indonesia: "8 Agustus 2026" (WIB).
///
/// Bentuk panjang, bukan `fmt_dt_full`: artikel bertanggal terbit, bukan
/// berjam-menit, dan halaman publik dibaca orang luar yang tak mengenal
/// singkatan bulan yang dipakai di dalam portal.
pub fn tanggal_panjang(ts: DateTime<Utc>) -> String {
    let d = ts.with_timezone(&wib()).date_naive();
    format!("{} {} {}", d.day(), BULAN_PANJANG[d.month0() as usize], d.year())
}

/// Rentang izin: dari kapan sampai kapan, lengkap dengan jamnya.
///
/// Satu fungsi untuk SEMUA layar (santri, wali kelas, pantauan staf) — bentuk
/// tanggal yang ditulis ulang di tiap halaman cepat atau lambat berbeda.
///
/// Izin sehari penuh disimpan 00:00 → 23:59:59 (migrasi 86); jamnya sengaja
/// TIDAK ditampilkan di kasus itu — "13 Agu 2026, 00:00 – 23:59" hanya
/// membuat orang mengira ada jam yang perlu diperhatikan.
pub fn fmt_rentang(mulai: chrono::NaiveDateTime, selesai: chrono::NaiveDateTime) -> String {
    let f = |d: chrono::NaiveDate| {
        format!("{} {} {}", d.day(), BULAN_PENDEK[d.month0() as usize], d.year())
    };
    let sehari_penuh = mulai.time() == chrono::NaiveTime::MIN
        && selesai.time() >= chrono::NaiveTime::from_hms_opt(23, 59, 0).expect("23:59 valid");
    let sama_hari = mulai.date() == selesai.date();
    match (sama_hari, sehari_penuh) {
        (true, true) => f(mulai.date()),
        (true, false) => format!(
            "{}, {} – {} WIB",
            f(mulai.date()),
            mulai.format("%H:%M"),
            selesai.format("%H:%M")
        ),
        (false, true) => format!("{} – {}", f(mulai.date()), f(selesai.date())),
        (false, false) => format!(
            "{} {} → {} {} WIB",
            f(mulai.date()),
            mulai.format("%H:%M"),
            f(selesai.date()),
            selesai.format("%H:%M")
        ),
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

#[cfg(test)]
mod tests_rentang {
    use super::fmt_rentang;
    use chrono::{NaiveDate, NaiveTime};

    fn dt(t: &str, j: &str) -> chrono::NaiveDateTime {
        NaiveDate::parse_from_str(t, "%Y-%m-%d")
            .unwrap()
            .and_time(NaiveTime::parse_from_str(j, "%H:%M:%S").unwrap())
    }

    /// Sehari penuh disimpan 00:00 → 23:59:59 (migrasi 86). Jamnya sengaja TAK
    /// ditampilkan: "13 Agu 2026, 00:00 – 23:59" hanya membuat orang mengira
    /// ada jam yang perlu diperhatikan.
    #[test]
    fn sehari_penuh_tanpa_jam() {
        assert_eq!(
            fmt_rentang(dt("2026-08-13", "00:00:00"), dt("2026-08-13", "23:59:59")),
            "13 Agu 2026"
        );
    }

    #[test]
    fn sehari_dengan_jam() {
        assert_eq!(
            fmt_rentang(dt("2026-08-13", "09:00:00"), dt("2026-08-13", "12:00:00")),
            "13 Agu 2026, 09:00 – 12:00 WIB"
        );
    }

    #[test]
    fn beberapa_hari_penuh() {
        assert_eq!(
            fmt_rentang(dt("2026-08-13", "00:00:00"), dt("2026-08-15", "23:59:59")),
            "13 Agu 2026 – 15 Agu 2026"
        );
    }

    /// Kasus yang paling perlu jelas: jam keluar milik tanggal keluar, jam
    /// kembali milik tanggal kembali.
    #[test]
    fn lintas_hari_dengan_jam() {
        assert_eq!(
            fmt_rentang(dt("2026-08-13", "14:00:00"), dt("2026-08-15", "08:00:00")),
            "13 Agu 2026 14:00 → 15 Agu 2026 08:00 WIB"
        );
    }
}
