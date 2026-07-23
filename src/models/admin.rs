//! models/admin.rs — Payload halaman "User Control" (admin-only, migrasi 17:
//! activity_logs). Manajemen user (aktif/nonaktif, ganti peran) + jejak aksi.

use serde::{Deserialize, Serialize};

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
