//! models/attendance.rs — Tipe absensi: item riwayat, antrean verifikasi, scan RFID.

use serde::{Deserialize, Serialize};

/// Aturan poin kehadiran GLOBAL — SATU sumber untuk tampilan & pemberian poin
/// saat verifikasi disetujui: (delta, label tampilan, kategori point_logs).
/// present +10 Kedisiplinan · late +2 Kedisiplinan · permit/sick 0 Keterangan ·
/// absent -15 Pelanggaran. Bisa di-override PER JADWAL (class_schedules) di
/// repository: `late_points` (migrasi 13, delta bertanda langsung) untuk
/// 'late', `absent_points` (migrasi 15, magnitude positif, `points - absent_points`)
/// untuk 'absent' — lihat repository::run_auto_absent & run_auto_verify_pamong.
pub fn point_rule(status: &str) -> (i32, &'static str, &'static str) {
    match status {
        "present" => (10, "Kedisiplinan", "attendance"),
        "late" => (2, "Kedisiplinan", "attendance"),
        // Hadir tapi di luar jadwal: netral (pamong/dewan guru yang menilai).
        "outside_schedule" => (0, "Di luar jadwal", "attendance"),
        "permit" | "sick" => (0, "Keterangan", "attendance"),
        _ => (-15, "Pelanggaran", "discipline"),
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
