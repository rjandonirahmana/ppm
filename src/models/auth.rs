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
        // 'teacher' digabung ke 'dewan_guru' di migrasi 36, dan sejak migrasi
        // 84 tak lagi sah di kolom `users.role`. Ia hanya bisa muncul dari
        // KLAIM TOKEN lama, dan hanya terpakai saat DB tak menjawab — di jalur
        // itulah `require_session` jatuh kembali ke klaim.
        //
        // Ditangani DI SINI, bukan dengan menulis "teacher" di setiap daftar
        // peran: cara lama membuat delapan endpoint lupa menulisnya, sehingga
        // akun lama diterima di sebagian layar dan ditolak di sebagian lain.
        //
        // ('supervisor' — bekas pamong — dibuang seluruhnya: migrasi 84 sudah
        // mengubah akunnya jadi 'dewan_guru', dan peran dibaca segar dari DB.)
        || (role == "teacher" && allowed.contains(&"dewan_guru"))
}

pub fn role_home(role: &str) -> &'static str {
    match role {
        // Ketua = admin + finance → dashboard staf (admin).
        "admin" | "ketua" => "/staf",
        // 'teacher' digabung ke 'dewan_guru' (migrasi 36) — arahkan ke dashboard
        // yang sama bila ada sisa data lama.
        "teacher" | "dewan_guru" => "/dewan-guru",
        // santri_finance = santri pemegang kunci finance → dashboard santri.
        "santri" | "santri_finance" => "/santri",
        "parent" => "/orang-tua",
        // Penjaga hanya punya satu pekerjaan; layar itu sekaligus berandanya.
        "penjaga" => "/tamu-masuk",
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
        "santri" => "Santri",
        "santri_finance" => "Santri (Finance)",
        "parent" => "Orang Tua",
        "penjaga" => "Penjaga",
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
/// Mengundang seseorang jadi `dewan_guru` = memberi wewenang
/// setara atau lebih tinggi dari pengundang. Tanpa batas ini, pamong bisa
/// mencetak link yang menjadikan siapa pun dewan guru tanpa sepengetahuan
/// admin. Mengundang santri/orang tua tetap boleh — itu tugas harian mereka.
/// `penjaga` ikut di sini meski wewenangnya paling sempit: ia tetap AKUN
/// PETUGAS yang bisa membaca nama, nomor HP, dan foto wajah tamu. Yang menahan
/// pamong/dewan guru mencetaknya bukan besarnya wewenang, melainkan bahwa
/// menambah petugas adalah keputusan pengurus — bukan efek samping dari
/// seseorang yang kebetulan punya tombol undangan.
pub const STAFF_INVITABLE_ROLES: &[&str] = &["dewan_guru", "penjaga"];

/// true bila `target_role` termasuk peran staf (khusus admin yang mengundang).
pub fn is_staff_invite(target_role: &str) -> bool {
    STAFF_INVITABLE_ROLES.contains(&target_role)
}

/// Boleh tidaknya `by_role` mencetak undangan untuk `target_role`.
pub fn can_invite(by_role: &str, target_role: &str) -> bool {
    !is_staff_invite(target_role) || role_satisfies(by_role, &["admin"])
}

/// Peran yang hanya boleh DIBERIKAN dan DICABUT oleh ketua sendiri.
///
/// `ketua` bukan sekadar peran dengan menu lebih banyak: ia admin PENUH
/// ditambah seluruh kewenangan keuangan (lihat [`role_satisfies`]) dan
/// satu-satunya yang boleh menghapus akun beserta datanya. Membiarkan admin
/// biasa memberikannya berarti setiap admin sesungguhnya SUDAH ketua — cukup
/// satu klik untuk mengangkat dirinya lewat akun lain, dan pemisahan wewenang
/// yang dijaga di seluruh berkas ini tinggal hiasan.
pub const KETUA_ONLY_ROLES: &[&str] = &["ketua"];

/// Boleh tidaknya `by_role` mengubah peran seseorang dari `current_role`
/// menjadi `new_role`.
///
/// Dijaga DUA ARAH, dan arah keduanya sama pentingnya:
///   • MENGANGKAT — hanya ketua yang menunjuk ketua;
///   • MENURUNKAN — hanya ketua pula yang boleh mencabutnya. Tanpa itu, admin
///     yang tak bisa mengangkat siapa pun tetap bisa MENYINGKIRKAN ketua yang
///     ada, dan kunci yang dipasang di arah pertama tak menahan apa-apa.
///
/// Tinggal di `models` (seperti [`can_invite`]) supaya dropdown peran di WASM
/// menyaring pilihan yang persis sama dengan yang ditegakkan server.
pub fn can_change_role(by_role: &str, current_role: &str, new_role: &str) -> bool {
    by_role == "ketua"
        || (!KETUA_ONLY_ROLES.contains(&current_role) && !KETUA_ONLY_ROLES.contains(&new_role))
}
