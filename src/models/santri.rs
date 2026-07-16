//! models/santri.rs — Payload halaman santri: riwayat, izin, profil.

use serde::{Deserialize, Serialize};

// ── Riwayat kehadiran ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiwayatItem {
    /// Nama kelas/sesi (fallback: label gerbang).
    pub title: String,
    /// "21 Okt 2025, 20:00 WIB"
    pub time_label: String,
    /// HADIR | TERLAMBAT | IZIN | ALPA
    pub status_label: String,
    /// present|late|permit|absent → warna kartu.
    pub kind: String,
    /// Poin tampilan (aturan models::attendance::point_rule).
    pub points: i32,
    /// "Kedisiplinan" / "Keterangan" / "Pelanggaran"
    pub points_note: String,
    /// Grup bulan, mis. "Oktober 2025".
    pub month: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RiwayatData {
    /// Hadir (present+late) semester ini.
    pub hadir: i64,
    /// Izin (permit+sick) semester ini.
    pub izin: i64,
    /// Alpa (absent) semester ini.
    pub alpa: i64,
    /// Label semester, mis. "Semester Ganjil 25/26".
    pub semester_label: String,
    pub items: Vec<RiwayatItem>,
}

// ── Izin / perizinan ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PermitItem {
    /// "Izin Sakit" / "Izin Pulang" / "Izin Lainnya"
    pub kind_label: String,
    /// "12 – 13 Nov 2025"
    pub range_label: String,
    /// "Menunggu" / "Disetujui" / "Ditolak"
    pub status_label: String,
    /// pending|approved|rejected → warna badge.
    pub status_kind: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IzinData {
    /// Persentase kehadiran semester ini.
    pub pct: i32,
    pub hadir: i64,
    pub absen: i64,
    pub points: i32,
    /// "Halaqah Subuh • 05:12 WIB" — scan terakhir hari ini (bila ada).
    pub detected: Option<String>,
    pub permits: Vec<PermitItem>,
}

// ── Profil ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilData {
    pub name: String,
    pub username: String,
    /// Label peran tampilan, mis. "SANTRI".
    pub role_label: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nis: Option<String>,
    pub points: i32,
}
