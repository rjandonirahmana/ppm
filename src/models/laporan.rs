//! models/laporan.rs — Payload halaman /laporan (menggantikan item Profil di
//! navbar; peran menentukan bentuk laporan yang tampil).

use serde::{Deserialize, Serialize};

use super::hafalan::HafalanRankItem;

/// Satu baris riwayat poin ("Prestasi Terbaru" / "Pelanggaran & Teguran").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaporanPointItem {
    pub reason: String,
    pub delta: i32,
    /// "20 Jul 2026 • 14:30"
    pub date_label: String,
}

/// Baris tabel "Performa Kelas" (laporan admin/pamong).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClassPerfRow {
    pub name: String,
    pub member_count: i64,
    /// Persentase kehadiran 30 hari terakhir (0 bila belum ada data).
    pub attendance_pct: i64,
    /// "Sangat Baik" / "Baik" / "Perlu Perhatian"
    pub status_label: String,
    pub status_kind: String,
}

/// Titik grafik tren kehadiran mingguan (W1..W5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrendBar {
    pub label: String,
    pub pct: i64,
}

/// Laporan Institusi — admin & pamong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaporanAdminData {
    pub attendance_pct: i64,
    pub active_violation_points: i64,
    /// Santri yang statusnya sedang "di luar pondok" (gerbang RFID).
    pub santri_di_luar: i64,
    pub trend: Vec<TrendBar>,
    pub points_recent: Vec<RecentPointRow>,
    pub classes: Vec<ClassPerfRow>,
}

/// Baris "Ringkasan Poin" (dengan nama santri) — laporan institusi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecentPointRow {
    pub name: String,
    pub class_name: String,
    pub reason: String,
    pub delta: i32,
}

/// Ekstensi laporan guru/dewan guru (di atas AnalisisData yang sudah ada):
/// ranking hafalan "Santri Teladan".
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaporanGuruExtra {
    pub hafalan_top: Vec<HafalanRankItem>,
}

/// Laporan Santri untuk Orang Tua (anak yang sedang dipantau).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaporanOrtuData {
    pub child_id: i64,
    pub child_name: String,
    pub class_name: String,
    pub nis: String,
    pub hadir: i64,
    pub izin: i64,
    pub alpa: i64,
    pub attendance_pct: i64,
    pub points: i64,
    pub prestasi: Vec<LaporanPointItem>,
    pub pelanggaran: Vec<LaporanPointItem>,
    pub hafalan: Vec<super::hafalan::HafalanItem>,
    pub juz_count: i64,
    /// Status gerbang pondok saat ini: "in" | "out".
    pub gate_status: String,
    /// "20 Jul 2026 • 14:30" — kapan status gerbang terakhir berubah.
    pub gate_at_label: String,
}

/// Rapor Pribadi Santri.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LaporanSantriData {
    pub hadir: i64,
    pub izin: i64,
    pub alpa: i64,
    pub attendance_pct: i64,
    pub points: i64,
    pub prestasi: Vec<LaporanPointItem>,
    pub pelanggaran: Vec<LaporanPointItem>,
    pub hafalan: Vec<super::hafalan::HafalanItem>,
    pub juz_count: i64,
    pub gate_status: String,
    pub gate_at_label: String,
}
