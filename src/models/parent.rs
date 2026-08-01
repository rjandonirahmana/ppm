//! models/parent.rs — Payload sisi ORANG TUA: pencarian santri, koneksi
//! (butuh persetujuan santri), pantauan banyak anak, izin, riwayat.

use serde::{Deserialize, Serialize};

use super::santri::{PermitItem, RiwayatData};

/// Hasil pencarian santri (form "Cari Nama atau NIS").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentSearchItem {
    pub id: i64,
    pub name: String,
    pub nis: String,
    pub class_name: String,
}

/// Chip pemilih anak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildChip {
    pub id: i64,
    pub name: String,
}

/// Status kehadiran HARI INI anak terpilih.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TodayStatus {
    /// "Masuk Sekolah" / "Terlambat" / dst.
    pub label: String,
    /// "07:12 WIB"
    pub time: String,
    /// "Pintu Gerbang Utama"
    pub gate: String,
}

/// Panel pantauan satu anak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildMonitor {
    pub id: i64,
    pub name: String,
    pub nis: String,
    pub class_name: String,
    pub today: Option<TodayStatus>,
    /// Persentase kehadiran bulan ini.
    pub pct: i32,
    pub hadir: i64,
    pub terlambat: i64,
    pub absen: i64,
    pub permits: Vec<PermitItem>,
}

/// Permintaan koneksi yang masih menunggu (sisi orang tua).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingConn {
    pub id: i64,
    pub student_name: String,
    /// "Terkirim: 2 jam lalu"
    pub since_label: String,
}

// Migrasi 46: `PendingParentConfirm` DIHAPUS — orang tua tak lagi jadi tahap
// persetujuan izin. Alur kini: pamong kelas -> wali kelas, per kelas dilewati.

/// Payload beranda orang tua.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParentHome {
    /// Anak-anak yang SUDAH terhubung (bisa banyak).
    pub children: Vec<ChildChip>,
    /// Permintaan koneksi yang menunggu persetujuan santri.
    pub pending: Vec<PendingConn>,
    /// Pantauan anak terpilih (None bila belum ada anak terhubung).
    pub monitor: Option<ChildMonitor>,
}

/// Permintaan koneksi masuk (sisi SANTRI — untuk disetujui/ditolak).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConnRequest {
    pub id: i64,
    pub parent_name: String,
    pub since_label: String,
}

/// Satu izin milik anak (daftar izin sisi orang tua, lintas-anak).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParentPermitItem {
    pub child_name: String,
    pub kind_label: String,
    pub range_label: String,
    pub reason: String,
    pub status_label: String,
    pub status_kind: String,
    /// "2 jam yang lalu"
    pub when_label: String,
}

/// Riwayat kehadiran satu anak (sisi orang tua).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChildRiwayat {
    pub child: ChildChip,
    pub data: RiwayatData,
}
