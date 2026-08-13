//! models/attendance.rs — Tipe absensi: item riwayat, antrean verifikasi, scan RFID.

use serde::{Deserialize, Serialize};

/// Default MAGNITUDO poin (semua POSITIF) bila jadwal tak mengisi. Semantik
/// seragam (migrasi 21): present DITAMBAH, late & absent DIKURANGI — TAK ADA
/// nilai minus di DB/UI, hanya arah operasinya yang beda.
pub const DEFAULT_PRESENT_BONUS: i32 = 10;
pub const DEFAULT_LATE_PENALTY: i32 = 0;
pub const DEFAULT_ABSENT_PENALTY: i32 = 15;

/// Aturan poin kehadiran GLOBAL untuk TAMPILAN saja: delta bertanda, label,
/// kategori point_logs.
///
/// Ini perkiraan, bukan angka yang benar-benar diberikan: ia tak tahu
/// `activity_type` maupun override per-jadwal, jadi hasilnya bisa berbeda dari
/// poin yang tercatat di `point_logs`. Dipakai `service::santri` untuk mengisi
/// kolom poin di riwayat. Pemberian poin yang sesungguhnya dihitung di SQL —
/// lihat `repository::attendance::DELTA_SQL`, satu-satunya sumber kebenaran
/// aritmetika poin.
pub fn point_rule(status: &str) -> (i32, &'static str, &'static str) {
    match status {
        "present" => (DEFAULT_PRESENT_BONUS, "Kedisiplinan", "attendance"),
        "late" => (-DEFAULT_LATE_PENALTY, "Kedisiplinan", "discipline"),
        "permit" | "sick" => (0, "Keterangan", "attendance"),
        _ => (-DEFAULT_ABSENT_PENALTY, "Pelanggaran", "discipline"),
    }
}

/// Jenis kegiatan valid (PRD). Selain ini → preset "legacy".
pub const ACTIVITY_TYPES: &[(&str, &str)] = &[
    ("kbm", "KBM (Ngaji/Sambung)"),
    ("non_kbm", "Non-KBM (Apel, dll)"),
    ("piket", "Piket Harian"),
    ("apel_kepulangan", "Apel Kepulangan"),
];

/// Kata pada `point_logs.reason` untuk sebuah status kehadiran.
///
/// Murni penamaan — angkanya dihitung di SQL (`repository::attendance::DELTA_SQL`).
/// Preset poin per `activity_type` yang dulu ada di sini (`category_points`)
/// sudah dihapus: ia menyalin fungsi SQL `cat_default_points()` migrasi 28, dan
/// dua salinan aturan yang sama di dua bahasa cepat atau lambat menyimpang.
pub fn attendance_note(status: &str) -> &'static str {
    match status {
        "present" | "late" => "Kedisiplinan",
        "absent" => "Pelanggaran",
        "permit" => "Izin",
        // Sakit/Cuti (surat sah) TIDAK mengurangi poin (PRD hal. 7 NB).
        "sick" => "Sakit/Cuti",
        _ => "Keterangan",
    }
}

/// Satu santri untuk verifikasi kehadiran PER-SESI (di halaman detail sesi).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionVerifyItem {
    pub att_id: i64,
    pub name: String,
    pub nis: String,
    /// present|late|absent|permit|sick — untuk label/warna.
    pub status: String,
}

/// Data panel verifikasi kehadiran satu sesi (tahap sesuai peran pemanggil).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionVerifyData {
    /// "pamong" | "final".
    pub stage: String,
    /// "Verifikasi Pamong" | "Verifikasi Final".
    pub stage_label: String,
    pub items: Vec<SessionVerifyItem>,
}

/// Satu item riwayat kehadiran (tampilan dashboard/riwayat santri).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttendanceItem {
    /// "Hadir - Gate 1" / "Izin - Sakit" / ...
    pub title: String,
    /// "Hari ini, 15:45 WIB"
    pub sub: String,
    /// Teks badge: "Tepat Waktu" / "Terlambat" / "Menunggu" / ...
    pub badge: String,
    /// Jenis untuk warna/ikon: present|late|permit|sick|absent
    pub kind: String,
}

/// Satu antrean verifikasi pamong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PendingAtt {
    pub id: i64,
    pub name: String,
    pub nis: String,
    pub class_name: String,
    pub time_label: String,
    pub gate: String,
}

/// Payload halaman verifikasi pamong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PamongData {
    pub pending: Vec<PendingAtt>,
    pub approved_today: i64,
    /// Statistik hari ini (hero dashboard pamong/dewan).
    pub total_santri: i64,
    pub hadir_today: i64,
    pub pct: i32,
    /// Sesi hari ini (kartu "Kelas untuk Diverifikasi").
    pub today: Vec<super::kelas::LiveSesi>,
    /// Kehadiran terbaru.
    pub latest: Vec<super::kelas::LatestAtt>,
}

// ── Device RFID ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfidScanRequest {
    pub api_key: String,
    /// Nomor kartu (users.rfid_cards).
    pub card: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfidScanResponse {
    pub ok: bool,
    pub message: String,
    /// Nama santri (bila dikenal).
    pub student: Option<String>,
    /// present | late (bila tercatat).
    pub status: Option<String>,
}

// ── Device RFID — Gerbang UTAMA pondok (masuk/keluar, TERPISAH dari gerbang
// kelas) ─────────────────────────────────────────────────────────────────────

/// Respons scan gerbang pondok. Request memakai `RfidScanRequest` yang sama
/// (api_key + card) — firmware gerbang pondok cukup pukul URL berbeda.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateScanResponse {
    pub ok: bool,
    pub message: String,
    pub student: Option<String>,
    /// Arah HASIL toggle: "in" (baru masuk) | "out" (baru keluar).
    pub direction: Option<String>,
}

/// Satu baris santri yang berstatus "di luar pondok" — laporan admin/pamong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutsideRow {
    pub user_id: i64,
    pub name: String,
    pub nis: String,
    pub class_name: String,
    /// "20 Jul 2026 • 14:30 WIB"
    pub since_label: String,
}
