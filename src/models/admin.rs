//! models/admin.rs — Payload halaman "User Control" (admin-only, migrasi 17:
//! activity_logs). Manajemen user (aktif/nonaktif, ganti peran) + jejak aksi.

use serde::{Deserialize, Serialize};

/// Perangkat RFID = "ruang" (dropdown jadwal + manajemen di User Control).
/// api_key dipakai firmware ESP8266 utk autentikasi POST /api/rfid/scan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RfidDeviceItem {
    pub id: i64,
    pub device_name: String,
    pub serial_number: String,
    pub location: String,
    pub api_key: String,
    /// gate_utama|gedung_putra|gedung_putri|masjid|custom (migrasi 49).
    pub category: String,
}

/// Kategori perangkat RFID: (nilai DB, label tampilan). SATU sumber kebenaran —
/// dipakai dropdown admin & label baris. Sinkron dgn CHECK constraint migrasi 49.
pub const DEVICE_CATEGORIES: &[(&str, &str)] = &[
    ("gate_utama", "Gerbang Utama"),
    ("gedung_putra", "Gedung Putra"),
    ("gedung_putri", "Gedung Putri"),
    ("masjid", "Masjid"),
    ("custom", "Lainnya"),
];

pub fn device_category_label(c: &str) -> &'static str {
    DEVICE_CATEGORIES
        .iter()
        .find(|(v, _)| *v == c)
        .map(|(_, l)| *l)
        .unwrap_or("Lainnya")
}

/// true bila perangkat ini GERBANG UTAMA — tap di sini berarti santri
/// KELUAR/MASUK area PPM (toggle status), BUKAN absensi kelas terjadwal.
pub fn is_main_gate(category: &str) -> bool {
    category == "gate_utama"
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserRow {
    pub id: i64,
    pub name: String,
    /// Peran mentah (admin/teacher/dewan_guru/supervisor/santri/parent) — utk
    /// pre-select dropdown ganti peran.
    pub role: String,
    /// "Admin" / "Guru" / "Dewan Guru" / "Pamong" / "Santri" / "Orang Tua".
    pub role_label: String,
    /// Email/username, atau NIS bila santri.
    pub contact: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserControlData {
    pub total: i64,
    pub santri_count: i64,
    /// Guru + dewan guru + pamong.
    pub staff_count: i64,
    pub inactive_count: i64,
    pub users: Vec<UserRow>,
}

/// Satu baris jejak aksi administratif (activity_logs, migrasi 17).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityLogItem {
    pub actor_name: String,
    pub target_name: Option<String>,
    /// "Ganti Peran" / "Nonaktifkan Akun" / dst — label ramah dari `action`.
    pub action_label: String,
    pub detail: Option<String>,
    pub when_label: String,
}

/// Satu kartu RFID tak dikenal yang menunggu dipasangkan ke pengguna.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingCardItem {
    pub card: i64,
    /// Perangkat tempat kartu terakhir ditempel.
    pub device: String,
    /// "Baru saja" / "5 menit lalu" — kapan terakhir ditempel.
    pub when_label: String,
    /// Epoch detik, untuk mengurutkan terbaru dulu (tak ditampilkan).
    pub sort_key: i64,
}

/// Hasil pencarian pengguna untuk pemasangan kartu — SEMUA peran, bukan santri
/// saja (pamong & dewan guru juga menempel kartu di gerbang).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserPickItem {
    pub id: i64,
    pub full_name: String,
    pub role_label: String,
    /// NIS bila santri, "-" bila bukan.
    pub nis: String,
    /// Nomor kartu terpasang saat ini (0 = belum punya).
    pub current_card: i64,
}
