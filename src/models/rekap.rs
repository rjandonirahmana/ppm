//! models/rekap.rs — Rekap kehadiran mingguan per-santri (laporan kontrol staf).

use serde::{Deserialize, Serialize};

/// Kategori Prestasi Ketertiban dari sisa saldo poin (PRD hal. 11).
/// Return (label, kind warna). Dipakai tampilan profil santri & rekap.
pub fn prestasi_label(points: i32) -> (&'static str, &'static str) {
    match points {
        p if p > 1050 => ("Istimewa", "success"),
        p if p >= 750 => ("Sangat Baik", "success"),
        p if p >= 451 => ("Baik", "info"),
        p if p >= 151 => ("Cukup", "primary"),
        p if p >= 101 => ("Kurang", "warning"),
        p if p >= 1 => ("Sangat Kurang", "error"),
        _ => ("Habis", "error"),
    }
}

/// Satu baris rekap: kehadiran satu santri untuk satu pekan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyRecapRow {
    pub name: String,
    pub nis: String,
    pub class_name: String,
    /// Angkatan (4 digit awal NIS) — untuk filter; "-" bila tak ada.
    pub angkatan: String,
    pub hadir: i64,
    pub telat: i64,
    pub izin: i64,
    pub alpa: i64,
    /// Persentase kehadiran = (hadir+telat) / total * 100.
    pub pct: i32,
    /// Sisa saldo poin santri (untuk prestasi ketertiban).
    pub points: i32,
}

/// Satu santri yang perlu dipanggil pekan ini (PRD hal. 12: tier berdasar total
/// net poin per pekan). Semakin minus → pemanggil semakin tinggi.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PemanggilanItem {
    pub name: String,
    pub nis: String,
    pub class_name: String,
    /// Total net poin pekan ini (negatif).
    pub net: i32,
    /// "KoorSantri" | "Wali Kelas" | "Ketua".
    pub tier: String,
    /// koor|wali|ketua → warna badge.
    pub tier_kind: String,
}

/// Pemanggil sesuai net poin mingguan (PRD hal. 12): net ≤ -18 → Ketua;
/// ≤ -12 → Wali Kelas; ≤ -9 → KoorSantri.
///
/// Tingkat tengahnya dulu PAMONG. Perannya dihapus (migrasi 84), jadi jenjangnya
/// digeser satu tingkat atas keputusan pengurus (Ags 2026): yang dulu ditangani
/// pamong kini jadi urusan WALI KELAS, dan yang paling berat naik ke KETUA.
/// AMBANGNYA TIDAK BERUBAH — yang bergeser siapa yang memanggil, bukan seberapa
/// minus seorang santri harus jatuh dulu.
pub fn pemanggilan_tier(net: i32) -> (&'static str, &'static str) {
    if net <= -18 {
        ("Ketua", "ketua")
    } else if net <= -12 {
        ("Wali Kelas", "wali")
    } else {
        ("KoorSantri", "koor")
    }
}

/// Tingkat SP dari sisa saldo poin (PRD hal. 14): ≤50 SP3, ≤100 SP2, ≤150 SP1,
/// >150 tak SP. Return (label, kind, penanganan singkat) atau None.
pub fn sp_level(points: i32) -> Option<(&'static str, &'static str, &'static str)> {
    if points <= 50 {
        Some(("SP 3", "error", "Santri + Ortu/Wali · Pimpinan Pondok"))
    } else if points <= 100 {
        Some(("SP 2", "error", "Santri + Ortu/Wali · BK + Pengurus"))
    } else if points <= 150 {
        Some(("SP 1", "warning", "Santri · BK + Pengurus"))
    } else {
        None
    }
}

/// Satu santri berstatus SP (untuk daftar penanganan khusus staf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpItem {
    pub name: String,
    pub nis: String,
    pub class_name: String,
    pub points: i32,
    /// "SP 1" | "SP 2" | "SP 3".
    pub level: String,
    /// warning|error → warna badge.
    pub level_kind: String,
    /// Penanganan singkat sesuai tingkat.
    pub treatment: String,
}

/// Reward mingguan PRD per JENIS kegiatan: (No-Alfa, No-Telat, Full-Hadir).
/// KBM 5/13/20 · Non-KBM 3/8/12 · Piket 2/0/0 · lainnya 0. MENCERMINKAN PRD hal. 8.
pub fn weekly_reward_points(activity_type: &str) -> (i32, i32, i32) {
    match activity_type {
        "kbm" => (5, 13, 20),
        "non_kbm" => (3, 8, 12),
        "piket" => (2, 0, 0),
        _ => (0, 0, 0),
    }
}

/// Satu baris reward mingguan santri (hasil perhitungan, siap dikreditkan).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyRewardRow {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    /// Total poin reward pekan ini.
    pub points: i32,
    /// Rincian, mis. "KBM: Full Hadir +20; Non-KBM: No Alfa +3".
    pub detail: String,
    /// Sudah dikreditkan (masuk saldo) atau belum.
    pub credited: bool,
}

/// Payload halaman /rekap-mingguan.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WeeklyRecapData {
    /// "21 – 27 Jul 2026"
    pub week_label: String,
    /// Offset pekan (0 = pekan ini, 1 = pekan lalu, dst) — untuk navigasi.
    pub offset: i32,
    /// Nama kelas unik (opsi filter).
    pub classes: Vec<String>,
    /// Angkatan unik (opsi filter).
    pub angkatans: Vec<String>,
    pub rows: Vec<WeeklyRecapRow>,
    pub total_santri: i64,
    /// Rata-rata persentase kehadiran seluruh santri pekan ini.
    pub avg_pct: i32,
    /// Reward mingguan per santri (hanya yang berhak, points > 0).
    pub rewards: Vec<WeeklyRewardRow>,
    /// Total poin reward semua santri pekan ini.
    pub rewards_total: i32,
    /// Jumlah santri yang rewardnya BELUM dikreditkan (untuk tombol admin).
    pub rewards_pending: i32,
    /// Pemanggilan mingguan (net poin ≤ -9) — daftar santri + tier pemanggil.
    pub pemanggilan: Vec<PemanggilanItem>,
    /// Santri berstatus SP (saldo ≤ 150) — penanganan khusus, saldo terendah dulu.
    pub sp_list: Vec<SpItem>,
}
