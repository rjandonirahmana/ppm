//! models/enums.rs — Type-safe enums untuk domain status/state.
//!
//! Replaces stringly-typed status strings dengan compile-time validated enums.
//! Typo di compile time, bukan runtime error.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Saldo poin awal santri (PRD "Sistem Poin 2.0": 300 poin — diberi tiap awal
/// semester, berkurang bila izin/alfa/telat).
///
/// Tinggal di `models`, bukan di `service::admin` seperti dulu, karena
/// `repository::insert_registered_user` juga membutuhkannya untuk mencatat
/// saldo awal ke buku besar. Repository mengimpor dari service akan membalik
/// arah lapisan (service yang memakai repository, bukan sebaliknya); `models`
/// memang lapisan bersama yang boleh dilihat keduanya.
pub const SEMESTER_START_POINTS: i32 = 300;

/// Status kehadiran santri.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttendanceStatus {
    /// Hadir tepat waktu.
    Present,
    /// Hadir tapi terlambat.
    Late,
    /// Scan di luar jam jadwal kelas.
    OutsideSchedule,
    /// Tidak hadir (alpa).
    Absent,
    /// Izin dengan surat.
    Permit,
    /// Sakit (dengan bukti).
    Sick,
}

impl AttendanceStatus {
    /// Konversi dari string (untuk parsing dari DB/API).
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "present" => Some(Self::Present),
            "late" => Some(Self::Late),
            "outside_schedule" => Some(Self::OutsideSchedule),
            "absent" => Some(Self::Absent),
            "permit" => Some(Self::Permit),
            "sick" => Some(Self::Sick),
            _ => None,
        }
    }

    /// Konversi ke string (untuk simpan ke DB/API).
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Present => "present",
            Self::Late => "late",
            Self::OutsideSchedule => "outside_schedule",
            Self::Absent => "absent",
            Self::Permit => "permit",
            Self::Sick => "sick",
        }
    }
}

impl fmt::Display for AttendanceStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Status approval izin (multi-stage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    /// Menunggu approval.
    Pending,
    /// Di-approve (lolos tahap ini).
    Approved,
    /// Di-reject (tidak disetujui).
    Rejected,
}

impl ApprovalStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }
}

impl fmt::Display for ApprovalStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Tipe izin/perizinan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermitKind {
    /// Izin karena sakit.
    Sick,
    /// Izin keperluan/cuti.
    Leave,
    /// Izin keperluan mendesak.
    Keperluan,
    /// Tipe izin lain.
    Other,
}

impl PermitKind {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "sick" => Some(Self::Sick),
            "leave" => Some(Self::Leave),
            "keperluan" => Some(Self::Keperluan),
            "other" => Some(Self::Other),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sick => "sick",
            Self::Leave => "leave",
            Self::Keperluan => "keperluan",
            Self::Other => "other",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Sick => "Sakit",
            Self::Leave => "Cuti",
            Self::Keperluan => "Keperluan",
            Self::Other => "Lainnya",
        }
    }
}

impl fmt::Display for PermitKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Status pembayaran tagihan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillStatus {
    /// Belum dibayar.
    Unpaid,
    /// Sudah lunas.
    Paid,
}

impl BillStatus {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "belum" | "unpaid" => Some(Self::Unpaid),
            "lunas" | "paid" => Some(Self::Paid),
            _ => None,
        }
    }

    /// Konversi ke format database (belum/lunas).
    pub fn as_db_str(&self) -> &'static str {
        match self {
            Self::Unpaid => "belum",
            Self::Paid => "lunas",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Unpaid => "Belum Bayar",
            Self::Paid => "Lunas",
        }
    }
}

impl fmt::Display for BillStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

/// Kategori poin (untuk point_logs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointCategory {
    /// Pencapaian positif (reward).
    Achievement,
    /// Masalah disiplin (penalty).
    Discipline,
    /// Poin kehadiran.
    Attendance,
    /// Poin manual (admin/guru input).
    Manual,
}

impl PointCategory {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "achievement" => Some(Self::Achievement),
            "discipline" => Some(Self::Discipline),
            "attendance" => Some(Self::Attendance),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Achievement => "achievement",
            Self::Discipline => "discipline",
            Self::Attendance => "attendance",
            Self::Manual => "manual",
        }
    }
}

impl fmt::Display for PointCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attendance_status_roundtrip() {
        assert_eq!(
            AttendanceStatus::from_str(AttendanceStatus::Present.as_str()),
            Some(AttendanceStatus::Present)
        );
        assert_eq!(
            AttendanceStatus::from_str(AttendanceStatus::Late.as_str()),
            Some(AttendanceStatus::Late)
        );
    }

    #[test]
    fn test_bill_status_roundtrip() {
        assert_eq!(
            BillStatus::from_str(BillStatus::Unpaid.as_db_str()),
            Some(BillStatus::Unpaid)
        );
        assert_eq!(
            BillStatus::from_str(BillStatus::Paid.as_db_str()),
            Some(BillStatus::Paid)
        );
    }

    #[test]
    fn test_invalid_status() {
        assert_eq!(AttendanceStatus::from_str("invalid_status"), None);
        assert_eq!(BillStatus::from_str("maybe"), None);
    }
}
