//! models/schedule.rs — Tipe jadwal kelas (tampilan).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScheduleInfo {
    pub title: String,
    pub class_name: String,
    /// "Hari ini, 04:30 WIB" / "Besok, 04:30 WIB"
    pub time_label: String,
}

/// Satu sesi kelas (halaman /sesi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionItem {
    pub id: i64,
    pub title: String,
    pub class_name: String,
    /// "Hari ini" / "Besok" / "16 Jul 2026"
    pub date_label: String,
    /// "04:30 WIB" / "-"
    pub time_label: String,
    /// Terjadwal | Berlangsung | Selesai | Dibatalkan
    pub status_label: String,
    /// scheduled|ongoing|finished|cancelled
    pub status_kind: String,
    pub teacher: String,
}

/// Payload halaman /sesi (nav dipilih dari role).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionsData {
    pub role: String,
    /// true = melihat SEMUA sesi (admin/pamong/dewan guru).
    pub all_scope: bool,
    pub items: Vec<SessionItem>,
}
