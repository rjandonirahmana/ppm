//! models/kelas.rs — Payload sisi STAF: dashboard, manajemen kelas, tinjau izin.

use serde::{Deserialize, Serialize};

/// Sesi live di dashboard staf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LiveSesi {
    pub title: String,
    pub teacher: String,
    pub santri_count: i64,
}

/// Dashboard staf (admin/dewan guru/pamong).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StafHome {
    pub name: String,
    pub total_santri: i64,
    pub hadir_today: i64,
    /// Persentase kehadiran hari ini.
    pub pct: i32,
    pub izin_pending: i64,
    pub live: Vec<LiveSesi>,
}

/// Satu kelas di halaman Manajemen Kelas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasItem {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// Pengajar sesi terakhir kelas ini (kolom teacher tak ada di classes).
    pub teacher: String,
    pub member_count: i64,
    pub schedule_count: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KelasData {
    pub total_kelas: i64,
    pub total_santri: i64,
    pub items: Vec<KelasItem>,
}

/// Anggota kelas (LIHAT SANTRI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MemberItem {
    pub id: i64,
    pub name: String,
    pub nis: String,
}

/// Izin menunggu peninjauan (halaman Tinjau Izin staf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermitReviewItem {
    pub id: i64,
    pub student_name: String,
    pub kind_label: String,
    pub range_label: String,
    pub reason: String,
    pub when_label: String,
}
