//! models/dashboard.rs — Payload agregat halaman dashboard.

use serde::{Deserialize, Serialize};

use super::attendance::AttendanceItem;
use super::schedule::ScheduleInfo;

/// Payload dashboard santri (/santri).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SantriHome {
    pub name: String,
    pub points: i32,
    pub schedule: Option<ScheduleInfo>,
    pub recent: Vec<AttendanceItem>,
    /// Persentase kehadiran bulan ini (None = belum ada catatan).
    pub month_pct: Option<i32>,
    /// Perubahan poin bulan berjalan (dari point_logs).
    pub month_points: i64,
    /// Izin/sakit yang SEDANG BERLAKU — spanduk di beranda. None = masuk biasa.
    ///
    /// Ditaruh di beranda, bukan hanya di halaman izin: santri yang sedang
    /// sakit membuka aplikasi dari beranda, dan yang perlu ia ketahui lebih
    /// dulu bukan poinnya melainkan sampai kapan izinnya berlaku.
    #[serde(default)]
    pub izin_aktif: Option<super::santri::SedangIzinItem>,
}
