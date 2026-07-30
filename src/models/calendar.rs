//! models/calendar.rs — Kalender akademik bulanan: sesi kelas per hari dalam
//! satu bulan, di-scope peran (staf = semua kelas; santri/ortu = kelasnya
//! sendiri). Struktur grid (leading_blanks + days_in_month) dihitung di server
//! agar klien cukup me-render tanpa aritmetika tanggal.

use serde::{Deserialize, Serialize};

/// Satu sesi pada satu tanggal di dalam bulan yang ditampilkan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarItem {
    /// Tanggal (1..=31) di bulan ini.
    pub day: u32,
    pub session_id: i64,
    /// "04:40" atau "-" bila sesi ad-hoc tanpa jadwal.
    pub time_label: String,
    pub title: String,
    pub class_name: String,
    pub teacher: String,
    pub category: String,
    /// "scheduled" | "ongoing" | "finished" | "cancelled".
    pub status_kind: String,
    pub status_label: String,
}

/// Payload kalender satu bulan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalendarData {
    pub year: i32,
    /// 1..=12.
    pub month: u32,
    /// "Juli 2026".
    pub month_label: String,
    pub prev_year: i32,
    pub prev_month: u32,
    pub next_year: i32,
    pub next_month: u32,
    /// Sel kosong sebelum tanggal 1 (grid Senin-first, 0..=6).
    pub leading_blanks: u32,
    pub days_in_month: u32,
    /// Tanggal hari ini (1..=31) bila bulan berjalan; 0 bila bukan.
    pub today_day: u32,
    /// "Semua kelas" / "Kelas Anda" / "Kelas anak Anda".
    pub scope_label: String,
    /// Label semester akademik aktif (mis. "Semester Ganjil 2026/2027"),
    /// kosong bila admin belum menetapkan (migrasi 40).
    pub active_semester: String,
    pub items: Vec<CalendarItem>,
}

/// Satu semester akademik yang didefinisikan admin (migrasi 40).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemesterItem {
    pub id: i64,
    /// "ganjil" | "genap" (mentah).
    pub kind: String,
    /// "Ganjil" | "Genap".
    pub kind_label: String,
    pub year: i16,
    /// "Semester Ganjil 2026/2027".
    pub label: String,
    /// "2026-07-01".
    pub start_date: String,
    pub end_date: String,
    pub is_active: bool,
}
