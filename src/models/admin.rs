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
    /// Peran mentah (admin/teacher/dewan_guru/santri/parent) — utk
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
    /// Boleh MENGUBAH pengguna (peran, aktif/nonaktif, kartu, perangkat)?
    /// Hanya admin/ketua. Guru & pamong tetap boleh MEMBUKA halaman ini —
    /// mereka perlu membaca jejak aktivitas — tapi seluruh kendalinya terkunci.
    pub can_manage: bool,
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

// ── Manajemen User (/manajemen-user, admin & ketua) ──────────────────────────

/// Satu baris di halaman manajemen user — lebih kaya dari [`UserRow`], yang
/// hanya melayani daftar ringkas di /kontrol-pengguna.
///
/// Memuat identitas santri (NIS, angkatan, kampus) supaya pengelola bisa
/// memastikan ia menyunting orang yang benar tanpa membuka halaman lain: pada
/// daftar 512 santri hasil impor ada 90 nama yang muncul lebih dari sekali, dan
/// nama saja tidak cukup membedakan mereka.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ManagedUser {
    pub id: i64,
    pub full_name: String,
    pub role: String,
    pub role_label: String,
    pub is_active: bool,
    pub nis: Option<String>,
    pub phone_number: Option<String>,
    pub entry_year: Option<i16>,
    pub gender: Option<String>,
    pub campus: Option<String>,
    pub major: Option<String>,
    pub mubalegh_status: Option<String>,
    pub pendidikan_status: Option<String>,
    /// Status keanggotaan di PPM (migrasi 82). None/`aktif` = masih santri.
    /// Terpisah dari `is_active`, yang mengatur AKSES bukan riwayat.
    #[serde(default)]
    pub status_ppm: Option<String>,
    pub points: i32,
    /// Sudah punya catatan poin? Menentukan apakah pengaktifan perlu memberi
    /// saldo awal — lihat `repository::activate_user`.
    pub has_point_logs: bool,
}

/// Isian yang boleh diubah pengelola di halaman manajemen user.
///
/// Peran dan status aktif SENGAJA tidak di sini: keduanya punya jalurnya
/// sendiri karena membawa akibat lain (pengaktifan bisa memberi saldo awal,
/// ganti peran mengubah hak akses) dan tak boleh ikut berubah diam-diam saat
/// seseorang sekadar membetulkan ejaan nama.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfilEdit {
    pub full_name: String,
    pub nis: String,
    pub phone_number: String,
    pub entry_year: Option<i16>,
    pub gender: String,
    pub campus: String,
    pub major: String,
    pub mubalegh_status: String,
    pub pendidikan_status: String,
    /// Kosong = biarkan seperti apa adanya (masih santri aktif).
    #[serde(default)]
    pub status_ppm: String,
}

/// Satu anak yang tertaut ke akun orang tua — dipakai panel "Anak" di sheet
/// Edit Profil (/manajemen-user), hanya muncul bila perannya `parent`.
///
/// Membawa NIS & kelas, bukan nama saja: daftar induk pondok ini memuat 90 nama
/// yang muncul lebih dari sekali, dan melepas anak yang salah dari akun ortu
/// berarti orang tua itu kehilangan akses ke data anaknya tanpa tahu sebabnya.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnakOrtu {
    pub student_id: i64,
    pub full_name: String,
    /// "-" bila santri belum punya NIS.
    pub nis: String,
    /// "-" bila belum masuk kelas mana pun.
    pub class_name: String,
    /// "Terhubung" bila sudah disetujui, "Menunggu persetujuan santri" bila
    /// permintaan ortu belum dijawab anaknya.
    pub status_label: String,
    /// true = `connected`. Dipisah dari labelnya supaya layar tak perlu
    /// membandingkan teks untuk memilih warna lencana.
    pub terhubung: bool,
}

/// Satu ORANG TUA seorang santri — cerminan [`AnakOrtu`] dari sisi sebaliknya.
///
/// Ada dua layar karena ada dua arah pertanyaan yang sama-sama wajar: "siapa
/// saja anak akun ortu ini?" (dibuka dari akun ortu) dan "siapa saja orang tua
/// santri ini?" (dibuka dari akun santri). Keduanya menulis ke junction yang
/// SAMA (`parent_connections`), jadi menautkan dari sisi mana pun hasilnya
/// identik — satu santri boleh punya beberapa ortu, satu ortu beberapa anak.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrtuSantri {
    pub parent_id: i64,
    pub full_name: String,
    /// "-" bila akun ortu belum mengisi nomor HP.
    pub phone: String,
    pub status_label: String,
    pub terhubung: bool,
}

/// Label status kemubalighan (migrasi 73) untuk layar.
pub fn mubalegh_label(kode: &str) -> &'static str {
    match kode {
        "belum" => "Belum",
        "iya" => "Mubaligh",
        "tugasan" => "Mubaligh Tugasan",
        _ => "—",
    }
}

/// Label status keanggotaan PPM (migrasi 82) untuk layar.
pub fn status_ppm_label(kode: &str) -> &'static str {
    match kode {
        "aktif" => "Santri aktif",
        "lulus" => "Lulus PPM",
        "mengundurkan_diri" => "Mengundurkan diri",
        "pindah" => "Pindah",
        // Kosong/NULL berarti belum pernah diubah — dan itu sama artinya
        // dengan masih aktif, bukan "tidak diketahui".
        _ => "Santri aktif",
    }
}

/// Label status pendidikan (migrasi 73) untuk layar.
pub fn pendidikan_label(kode: &str) -> &'static str {
    match kode {
        "belum" => "Belum kuliah",
        "kuliah" => "Sedang kuliah",
        "sarjana" => "Sarjana",
        _ => "—",
    }
}

// ── Buku tamu: layar penjaga (migrasi 83) ────────────────────────────────────

/// Satu kunjungan tamu di layar penjaga.
///
/// Waktu sudah berupa label WIB, bukan timestamp: penjaga membaca "07.42",
/// bukan menghitung selisih zona waktu sendiri. Pola yang sama dipakai seluruh
/// payload layar di aplikasi ini.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TamuMasukItem {
    pub id: i64,
    pub name: String,
    pub phone: String,
    pub purpose: String,
    /// Foto wajah dari mesin gerbang. Kosong = mesin tak sempat memotret —
    /// justru baris yang paling perlu diperiksa penjaga.
    pub face_url: String,
    pub waktu_label: String,
    /// Sudah diperiksa penjaga?
    pub diperiksa: bool,
    /// Nama penjaga pemeriksa; kosong bila belum.
    pub diperiksa_oleh: String,
    /// Catatan kejanggalan. Kosong = dinyatakan cocok.
    pub catatan: String,
}

/// Payload layar /tamu-masuk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TamuMasukData {
    /// Menunggu diperiksa DALAM RENTANG yang sedang dilihat.
    pub belum_diperiksa: i64,
    /// Jumlah kunjungan yang cocok penyaring — bukan panjang `items` (yang
    /// hanya halaman pertama).
    pub total: i64,
    /// Nama rentang yang sedang aktif, mis. "Semester Ganjil 25/26" — hanya
    /// server yang tahu semester mana yang sedang berjalan (tabel migrasi 40).
    pub rentang_label: String,
    /// HALAMAN PERTAMA; sisanya lewat `tamu_masuk_page`.
    pub items: Vec<TamuMasukItem>,
}

/// Pilihan rentang waktu buku tamu. Nilai kirinya yang dikirim ke server.
///
/// Tinggal di `models` supaya layar (WASM) dan penerjemah tanggal (server)
/// membaca daftar yang SAMA — kode rentang yang diketik ulang di dua tempat
/// cepat atau lambat berbeda satu huruf, dan penyaringnya diam-diam jadi
/// "semua".
pub const RENTANG_TAMU: &[(&str, &str)] = &[
    ("hari_ini", "Hari ini"),
    ("7", "7 hari"),
    ("30", "30 hari"),
    ("semester", "Semester ini"),
    ("semua", "Semua"),
];
