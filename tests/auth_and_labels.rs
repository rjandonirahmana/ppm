//! tests/auth_and_labels.rs — Uji logika murni AUTH & LABEL (di luar PRD poin;
//! itu ada di `prd_rules.rs`). Deterministik, tanpa DB/jaringan:
//!   • normalize_phone  → inti login-pakai-HP & forgot-password.
//!   • role_home        → redirect landing per peran.
//!   • quality_label / is_mengaji_category → label hafalan.
//! Jalankan: `cargo test --test auth_and_labels`.

use ppm::models::{
    can_change_role, is_mengaji_category, quality_label, role_home, role_label, role_satisfies,
};
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
    assert!(!role_satisfies("ketua", &["dewan_guru"]));
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
    assert!(role_satisfies("dewan_guru", &["dewan_guru", "admin"]));
    assert!(!role_satisfies("santri", &["admin"]));
    assert!(!role_satisfies("parent", &["santri"]));
}

// ── JWT: benar-benar DITANDATANGANI, bukan sekadar meng-compile ──────────────
//
// Tes ini ada karena satu kegagalan yang lolos `cargo check` DAN seluruh tes
// lain: `jsonwebtoken` 11 tak lagi membawa penyedia kripto sendiri — fiturnya
// harus dipilih (`rust_crypto`/`aws_lc_rs`). Tanpa itu kodenya compile mulus
// lalu PANIC di `encode()` pertama, yaitu saat orang pertama menekan "Masuk",
// sebagai 500 dari server fn login. Memanggil sign+verify sungguhan di sini
// memindahkan kegagalan itu ke `cargo test`, tempat ia seharusnya ketahuan.

#[test]
fn jwt_sign_lalu_verify_mengembalikan_klaim_yang_sama() {
    let jwt = ppm::auth::JwtService::new("rahasia-untuk-tes");
    let token = jwt.sign(42, "Budi", "6281234567890", "ketua").expect("sign gagal");
    let klaim = jwt.verify(&token).expect("verify gagal");
    assert_eq!(klaim.user_id, 42);
    assert_eq!(klaim.name, "Budi");
    assert_eq!(klaim.phone, "6281234567890");
    assert_eq!(klaim.role, "ketua");
}

#[test]
fn jwt_token_asing_ditolak() {
    let jwt = ppm::auth::JwtService::new("rahasia-untuk-tes");
    let token = ppm::auth::JwtService::new("rahasia-lain")
        .sign(1, "X", "628", "admin")
        .expect("sign gagal");
    // Tanda tangan dari secret lain → ditolak, bukan diterima diam-diam.
    assert!(jwt.verify(&token).is_err());
}

// ── can_change_role (siapa boleh menunjuk ketua) ─────────────────────────────
//
// `role_satisfies` sengaja hanya berlaku SATU ARAH — ketua memenuhi "admin",
// admin tidak memenuhi "ketua". Tanpa aturan tambahan di bawah, arah itu tak
// menolong sama sekali di halaman peran: admin lolos penjaga `["admin"]`, lalu
// bebas mengangkat siapa pun (termasuk akun keduanya sendiri) menjadi ketua —
// dan sesudah itu seluruh pemisahan admin↔ketua di aplikasi ini tak berarti.

#[test]
fn hanya_ketua_yang_mengangkat_ketua() {
    assert!(can_change_role("ketua", "santri", "ketua"));
    assert!(!can_change_role("admin", "santri", "ketua"));
    assert!(!can_change_role("dewan_guru", "santri", "ketua"));
}

#[test]
fn hanya_ketua_yang_mencabut_peran_ketua() {
    // Arah sebaliknya sama pentingnya: admin yang tak bisa mengangkat siapa pun
    // tetap bisa MENYINGKIRKAN ketua yang ada bila arah ini dibiarkan terbuka.
    assert!(can_change_role("ketua", "ketua", "santri"));
    assert!(!can_change_role("admin", "ketua", "santri"));
    assert!(!can_change_role("admin", "ketua", "admin"));
}

#[test]
fn peran_selain_ketua_tetap_urusan_admin() {
    // Yang dikunci HANYA peran ketua — pekerjaan harian admin tak berubah.
    assert!(can_change_role("admin", "santri", "dewan_guru"));
    assert!(can_change_role("admin", "dewan_guru", "admin"));
    assert!(can_change_role("admin", "parent", "penjaga"));
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

// ── Peran lama yang sudah tak sah di DB, tapi masih hidup di klaim JWT ───────
//
// `users.role` sejak migrasi 84 hanya menerima tujuh nilai; 'teacher'
// (digabung ke 'dewan_guru' di migrasi 36) bukan salah satunya. `require_session`
// membaca peran SEGAR dari DB, jadi klaim lama hanya terpakai di satu jalur:
// ketika DB tak menjawab dan sesinya jatuh kembali ke isi token.

/// 'teacher' harus diterima di mana pun 'dewan_guru' diterima. Ditangani di
/// `role_satisfies`, bukan dengan menulisnya di tiap daftar peran — cara lama
/// membuat delapan endpoint lupa menulisnya.
#[test]
fn teacher_setara_dewan_guru() {
    assert!(role_satisfies("teacher", &["dewan_guru"]));
    assert!(role_satisfies("teacher", &["admin", "dewan_guru"]));
}

/// Setara BUKAN berarti naik pangkat.
#[test]
fn teacher_tidak_naik_pangkat() {
    assert!(!role_satisfies("teacher", &["admin"]));
    assert!(!role_satisfies("teacher", &["ketua"]));
    assert!(!role_satisfies("teacher", &["santri"]));
}

/// 'supervisor' (pamong) DIBUANG SELURUHNYA. Peran itu tak lagi punya arti di
/// mana pun: bukan alias, bukan peran, bukan label.
///
/// Diuji supaya tak diam-diam dihidupkan lagi lewat daftar peran baru. Kalau
/// suatu saat pamong benar-benar dibutuhkan lagi, ia harus lahir sebagai
/// keputusan sadar — bukan sebagai sisa yang tak pernah dibersihkan.
#[test]
fn supervisor_tak_punya_wewenang_apa_pun() {
    for daftar in [
        &["dewan_guru"][..],
        &["admin"][..],
        &["ketua"][..],
        &["santri"][..],
        &["admin", "ketua", "dewan_guru", "santri", "parent", "penjaga"][..],
    ] {
        assert!(
            !role_satisfies("supervisor", daftar),
            "supervisor tak boleh lolos {daftar:?}"
        );
    }
    assert_eq!(role_label("supervisor"), "Pengguna");
}
