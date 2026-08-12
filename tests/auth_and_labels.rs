//! tests/auth_and_labels.rs — Uji logika murni AUTH & LABEL (di luar PRD poin;
//! itu ada di `prd_rules.rs`). Deterministik, tanpa DB/jaringan:
//!   • normalize_phone  → inti login-pakai-HP & forgot-password.
//!   • role_home        → redirect landing per peran.
//!   • quality_label / is_mengaji_category → label hafalan.
//! Jalankan: `cargo test --test auth_and_labels`.

use ppm::models::{is_mengaji_category, quality_label, role_home, role_satisfies};
use ppm::service::auth::normalize_phone;

// ── normalize_phone (login by phone + forgot-password) ───────────────────────

#[test]
fn normalize_08_jadi_62() {
    assert_eq!(normalize_phone("081234567890"), "6281234567890");
}

#[test]
fn normalize_buang_pemisah_umum() {
    // Spasi, strip, kurung, titik → semua dibuang sebelum normalisasi.
    assert_eq!(normalize_phone("0812-3456-7890"), "6281234567890");
    assert_eq!(normalize_phone("0812 3456 7890"), "6281234567890");
    assert_eq!(normalize_phone("(0812) 3456.7890"), "6281234567890");
}

#[test]
fn normalize_sudah_62_atau_plus62_tetap() {
    assert_eq!(normalize_phone("6281234567890"), "6281234567890");
    assert_eq!(normalize_phone("+62 812-3456-7890"), "6281234567890");
}

#[test]
fn normalize_kosong_dan_pendek() {
    // `normalize_phone` dipakai untuk MENCARI (login, lupa sandi), jadi masukan
    // yang tak bisa ditafsirkan dikembalikan sebagai digitnya saja — pencarian
    // berakhir "tak ada yang cocok", bukan galat.
    //
    // Berbeda dari sebelumnya: "08" dulu jadi "628" karena awalan nol dipangkas
    // tanpa memeriksa apa pun. Menyulap potongan angka jadi sesuatu yang
    // BERBENTUK nomor sah justru berbahaya — bentuk itu bisa tersimpan dan
    // mengunci nomor yang bukan milik siapa pun. Lihat `models::normalisasi_hp`.
    assert_eq!(normalize_phone(""), "");
    assert_eq!(normalize_phone("08"), "08");
    // Bukan nomor seluler Indonesia → apa adanya (digitnya saja).
    assert_eq!(normalize_phone("1555"), "1555");
    // Nomor rumah ikut ditolak: bukan diawali 8 setelah kode negara.
    assert_eq!(normalize_phone("0217654321"), "0217654321");
}

/// Cacat yang membuat OTP & pengingat gagal terkirim selamanya — nomor yang
/// ditulis dengan kode negara DAN angka nol daerah sekaligus.
#[test]
fn normalize_kode_negara_plus_nol_daerah() {
    assert_eq!(normalize_phone("+62 0812-3456-7890"), "6281234567890");
    // Yang penting: hasilnya tak pernah berawalan "620".
    assert!(!normalize_phone("+62 0812 3456 7890").starts_with("620"));
}

// ── role_home (redirect landing per peran) ───────────────────────────────────

#[test]
fn role_home_tiap_peran() {
    assert_eq!(role_home("admin"), "/staf");
    assert_eq!(role_home("ketua"), "/staf"); // ketua = admin + finance
    // 'teacher' digabung ke dewan_guru (migrasi 36) → dashboard sama.
    assert_eq!(role_home("teacher"), "/dewan-guru");
    assert_eq!(role_home("dewan_guru"), "/dewan-guru");
    assert_eq!(role_home("supervisor"), "/verifikasi-pamong");
    assert_eq!(role_home("santri"), "/santri");
    assert_eq!(role_home("santri_finance"), "/santri"); // santri + finance
    assert_eq!(role_home("parent"), "/orang-tua");
}

#[test]
fn role_home_tak_dikenal_ke_menu() {
    assert_eq!(role_home(""), "/menu");
    assert_eq!(role_home("random"), "/menu");
}

// ── role_satisfies (ketua=admin, santri_finance=santri) ─────────────────────

#[test]
fn ketua_setara_admin() {
    // Di endpoint yang mengizinkan admin, ketua ikut boleh.
    assert!(role_satisfies("ketua", &["admin", "dewan_guru"]));
    assert!(role_satisfies("ketua", &["admin"]));
    // Endpoint tanpa admin → ketua TIDAK otomatis boleh.
    assert!(!role_satisfies("ketua", &["dewan_guru", "supervisor"]));
    assert!(!role_satisfies("ketua", &["santri"]));
}

#[test]
fn santri_finance_setara_santri() {
    assert!(role_satisfies("santri_finance", &["santri", "admin"]));
    assert!(role_satisfies("santri_finance", &["santri"]));
    // Endpoint finance meng-list santri_finance eksplisit.
    assert!(role_satisfies("santri_finance", &["admin", "ketua", "santri_finance"]));
    // Bukan santri → tak boleh di gate staf.
    assert!(!role_satisfies("santri_finance", &["admin", "dewan_guru"]));
}

#[test]
fn role_biasa_cocok_persis() {
    assert!(role_satisfies("admin", &["admin"]));
    assert!(role_satisfies("dewan_guru", &["supervisor", "dewan_guru", "admin"]));
    assert!(!role_satisfies("supervisor", &["admin", "dewan_guru"]));
    assert!(!role_satisfies("santri", &["admin"]));
    assert!(!role_satisfies("parent", &["santri"]));
}

// ── Label hafalan ────────────────────────────────────────────────────────────

#[test]
fn quality_label_hafalan() {
    assert_eq!(quality_label("perlu_perbaikan"), "Perlu Perbaikan");
    assert_eq!(quality_label("mengulang"), "Mengulang");
    assert_eq!(quality_label("lancar"), "Lancar");
    assert_eq!(quality_label("apa_pun"), "Lancar"); // default
}

#[test]
fn kategori_mengaji_variasi() {
    assert!(is_mengaji_category("Mengaji Kitab"));
    assert!(is_mengaji_category("PENGAJIAN Subuh"));
    assert!(is_mengaji_category("Tahfidz"));
    assert!(is_mengaji_category("Setoran Hafalan"));
    assert!(!is_mengaji_category("Sholat Berjamaah"));
    assert!(!is_mengaji_category("Piket"));
    assert!(!is_mengaji_category(""));
}
