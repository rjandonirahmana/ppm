//! models/attendance.rs — Tipe absensi: item riwayat, antrean verifikasi, scan RFID.

use serde::{Deserialize, Serialize};

/// Default MAGNITUDO poin (semua POSITIF) bila jadwal tak mengisi. Semantik
/// seragam (migrasi 21): present DITAMBAH, late & absent DIKURANGI — TAK ADA
/// nilai minus di DB/UI, hanya arah operasinya yang beda.
pub const DEFAULT_PRESENT_BONUS: i32 = 10;
pub const DEFAULT_LATE_PENALTY: i32 = 0;
pub const DEFAULT_ABSENT_PENALTY: i32 = 15;

/// Aturan poin kehadiran GLOBAL (fallback tampilan tanpa konteks jadwal): delta
/// bertanda, label, kategori point_logs. Pemberian poin sesungguhnya memakai
/// `attendance_delta` dgn override per-jadwal — lihat repository::decide_pamong,
/// run_auto_verify_pamong, run_auto_absent.
pub fn point_rule(status: &str) -> (i32, &'static str, &'static str) {
    match status {
        "present" => (DEFAULT_PRESENT_BONUS, "Kedisiplinan", "attendance"),
        "late" => (-DEFAULT_LATE_PENALTY, "Kedisiplinan", "discipline"),
        // Hadir tapi di luar jadwal: netral (pamong/dewan guru yang menilai).
        "outside_schedule" => (0, "Di luar jadwal", "attendance"),
        "permit" | "sick" => (0, "Keterangan", "attendance"),
        _ => (-DEFAULT_ABSENT_PENALTY, "Pelanggaran", "discipline"),
    }
}

/// Delta poin akhir dari status + konfigurasi jadwal (semua param MAGNITUDO
/// POSITIF; None = pakai default). present → +present; late → −late; absent →
/// −absent; lainnya → 0. Mengembalikan (delta bertanda, label, kategori).
pub fn attendance_delta(
    status: &str,
    present: Option<i16>,
    late: Option<i16>,
    absent: Option<i16>,
) -> (i32, &'static str, &'static str) {
    let m = |v: Option<i16>, d: i32| v.map(|x| (x as i32).abs()).unwrap_or(d);
    match status {
        "present" => (m(present, DEFAULT_PRESENT_BONUS), "Kedisiplinan", "attendance"),
        "late" => (-m(late, DEFAULT_LATE_PENALTY), "Kedisiplinan", "discipline"),
        "absent" => (-m(absent, DEFAULT_ABSENT_PENALTY), "Pelanggaran", "discipline"),
        "outside_schedule" => (0, "Di luar jadwal", "attendance"),
        _ => (0, "Keterangan", "attendance"),
    }
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
