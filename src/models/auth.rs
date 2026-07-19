//! models/auth.rs — Tipe sesi login & routing peran.

use serde::{Deserialize, Serialize};

/// Claims JWT — bentuk SAMA dengan proyek e-ticketing (models/auth.rs):
/// { user_id, phone, role, name, exp }. Bedanya hanya tipe user_id (BIGSERIAL
/// → i64; e-ticketing pakai ULID String).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub user_id: i64,
    pub phone: String,
    pub role: String,
    pub name: String,
    pub exp: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionUser {
    pub id: i64,
    pub name: String,
    pub role: String,
}

impl From<Claims> for SessionUser {
    fn from(c: Claims) -> Self {
        SessionUser {
            id: c.user_id,
            name: c.name,
            role: c.role,
        }
    }
}

/// Rute default per peran setelah login.
pub fn role_home(role: &str) -> &'static str {
    match role {
        "admin" => "/staf",
        "teacher" => "/guru",
        "dewan_guru" => "/dewan-guru",
        "supervisor" => "/verifikasi-pamong",
        "santri" => "/santri",
        "parent" => "/orang-tua",
        _ => "/menu",
    }
}
