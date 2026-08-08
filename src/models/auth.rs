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
/// Apakah `role` memenuhi salah satu `allowed`? Aturan superset:
///   • `ketua` = admin PENUH → boleh di mana pun `admin` boleh.
///   • `santri_finance` = santri PENUH → boleh di mana pun `santri` boleh
///     (akses finance-nya di-list eksplisit di gate finance).
/// Dipakai `require_roles` (satu tempat) → tak perlu menambal tiap gate, dan
/// mencegah user finance ter-forbidden lalu "keluar session" (redirect /login).
pub fn role_satisfies(role: &str, allowed: &[&str]) -> bool {
    allowed.contains(&role)
        || (role == "ketua" && allowed.contains(&"admin"))
        || (role == "santri_finance" && allowed.contains(&"santri"))
}

pub fn role_home(role: &str) -> &'static str {
    match role {
        // Ketua = admin + finance → dashboard staf (admin).
        "admin" | "ketua" => "/staf",
        // 'teacher' digabung ke 'dewan_guru' (migrasi 36) — arahkan ke dashboard
        // yang sama bila ada sisa data lama.
        "teacher" | "dewan_guru" => "/dewan-guru",
        "supervisor" => "/verifikasi-pamong",
        // santri_finance = santri pemegang kunci finance → dashboard santri.
        "santri" | "santri_finance" => "/santri",
        "parent" => "/orang-tua",
        _ => "/menu",
    }
}

/// Label peran untuk layar. SATU sumber — dulu `match` yang sama ditulis ulang
/// di `service::admin`, `components::MobileHeader`, dan `DesktopSidebar`, dan
/// ketiganya sudah menyimpang: dua di antaranya mengenal "ketua" dan
/// "santri_finance", yang ketiga menampilkan keduanya sebagai "Pengguna".
///
/// Tinggal di `models` supaya `repository` juga bisa memakainya tanpa membalik
/// arah lapisan (repository → service).
pub fn role_label(role: &str) -> &'static str {
    match role {
        "admin" => "Admin",
        "ketua" => "Ketua",
        // 'teacher' digabung ke 'dewan_guru' (migrasi 36); sisa data lama tetap
        // diberi label yang benar alih-alih jatuh ke "Pengguna".
        "teacher" | "dewan_guru" => "Dewan Guru",
        "supervisor" => "Pamong",
        "santri" => "Santri",
        "santri_finance" => "Santri (Finance)",
        "parent" => "Orang Tua",
        _ => "Pengguna",
    }
}

/// true bila peran ini santri → wajib melengkapi profil mahasiswa saat daftar
/// (gender, kampus, jurusan, tahun masuk PPM). Santri PPM = mahasiswa kampus
/// sekitar, jadi data ini bagian dari identitas dasarnya, bukan pelengkap.
pub fn needs_student_profile(role: &str) -> bool {
    matches!(role, "santri" | "santri_finance")
}

/// Hasil validasi kode referal untuk halaman registrasi. Sengaja TIDAK membawa
/// nama peran mentah — klien cukup tahu labelnya dan apakah form perlu meminta
/// data mahasiswa.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InviteInfo {
    /// "Santri" / "Dewan Guru" / … — untuk "Anda akan didaftarkan sebagai: …".
    pub role_label: String,
    /// true → tampilkan isian gender/kampus/jurusan/tahun masuk PPM.
    pub needs_student_profile: bool,
}

/// Peran STAF yang hanya boleh DIUNDANG oleh admin.
///
/// Mengundang seseorang jadi `dewan_guru`/`supervisor` = memberi wewenang
/// setara atau lebih tinggi dari pengundang. Tanpa batas ini, pamong bisa
/// mencetak link yang menjadikan siapa pun dewan guru tanpa sepengetahuan
/// admin. Mengundang santri/orang tua tetap boleh — itu tugas harian mereka.
pub const STAFF_INVITABLE_ROLES: &[&str] = &["dewan_guru", "supervisor"];

/// true bila `target_role` termasuk peran staf (khusus admin yang mengundang).
pub fn is_staff_invite(target_role: &str) -> bool {
    STAFF_INVITABLE_ROLES.contains(&target_role)
}

/// Boleh tidaknya `by_role` mencetak undangan untuk `target_role`.
pub fn can_invite(by_role: &str, target_role: &str) -> bool {
    !is_staff_invite(target_role) || role_satisfies(by_role, &["admin"])
}
