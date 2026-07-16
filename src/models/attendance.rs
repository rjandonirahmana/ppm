//! models/attendance.rs — Tipe absensi: item riwayat, antrean verifikasi, scan RFID.

use serde::{Deserialize, Serialize};

/// Aturan poin kehadiran — SATU sumber untuk tampilan & pemberian poin saat
/// verifikasi disetujui: (delta, label tampilan, kategori point_logs).
/// present +10 Kedisiplinan · late +2 Kedisiplinan · permit/sick 0 Keterangan ·
/// absent -15 Pelanggaran.
pub fn point_rule(status: &str) -> (i32, &'static str, &'static str) {
    match status {
        "present" => (10, "Kedisiplinan", "attendance"),
        "late" => (2, "Kedisiplinan", "attendance"),
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
