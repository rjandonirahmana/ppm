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
    /// scheduled|ongoing|finished|cancelled ("cancelled" = libur)
    pub status_kind: String,
    pub teacher: String,
    /// Pengajar terpasang (untuk pre-select dropdown assign). None = belum diisi.
    pub teacher_id: Option<i64>,
}

/// Payload halaman /sesi (nav dipilih dari role).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionsData {
    pub role: String,
    /// true = melihat SEMUA sesi (admin/pamong/dewan guru).
    pub all_scope: bool,
    pub items: Vec<SessionItem>,
}

/// Satu baris absensi pada detail sesi (anggota kelas + status di sesi itu).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionAttRow {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    /// "HADIR"/"TERLAMBAT"/"ALPA"/… atau "BELUM TERCATAT"
    pub status_label: String,
    /// present|late|absent|permit|sick|none
    pub status_kind: String,
    /// "05:02 WIB" bila tercatat
    pub time_label: String,
}

/// Satu pesan chat sesi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionChatItem {
    pub name: String,
    pub message: String,
    pub time_label: String,
}

/// Payload halaman detail sesi /sesi/:id (staf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionDetailData {
    pub id: i64,
    pub title: String,
    pub class_name: String,
    pub date_label: String,
    pub time_label: String,
    pub status_label: String,
    pub status_kind: String,
    pub teacher: String,
    pub hadir: i64,
    pub total: i64,
    pub attendance: Vec<SessionAttRow>,
    pub chats: Vec<SessionChatItem>,
    /// URL/path rekaman bila sudah ada (kolom class_sessions.recording_path).
    pub recording_url: Option<String>,
    pub recording_label: String,
}

/// Payload ruang sesi live /sesi/:id/live (staf + santri peserta).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionLiveData {
    pub id: i64,
    pub title: String,
    pub class_name: String,
    pub teacher: String,
    /// scheduled|ongoing|finished|cancelled
    pub status_kind: String,
    /// true = boleh mulai/akhiri sesi (staf).
    pub can_manage: bool,
    pub chats: Vec<SessionChatItem>,
    /// Jumlah peserta kelas (indikator "128" di header).
    pub member_count: i64,
}
