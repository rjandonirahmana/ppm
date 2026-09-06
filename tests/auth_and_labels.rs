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

/// Dua peran dewan guru bertugas-tambahan (migrasi 93) mendarat di beranda
/// dewan guru — BUKAN jatuh ke `/menu` lewat cabang tak-dikenal, yang membuat
/// mereka mulai hari di layar yang bukan miliknya.
#[test]
fn role_home_dewan_guru_bertugas_tambahan() {
    assert_eq!(role_home("dewan_guru_finance"), "/dewan-guru");
    assert_eq!(role_home("dewan_guru_absensi"), "/dewan-guru");
}

/// Label menyebut tugas tambahannya, supaya pengelola tahu siapa memegang apa
/// dari daftar pengguna tanpa membuka layar lain.
#[test]
fn label_dewan_guru_bertugas_tambahan() {
    assert_eq!(role_label("dewan_guru_finance"), "Dewan Guru (Finance)");
    assert_eq!(role_label("dewan_guru_absensi"), "Dewan Guru (Absensi)");
    // Bukan jatuh ke "Pengguna".
    assert_ne!(role_label("dewan_guru_finance"), "Pengguna");
}

/// Keduanya adalah dewan guru SEPENUHNYA — tugas tambahannya menambah, tak
/// menggantikan. Kalau ini putus, mereka kehilangan seluruh layar dewan guru
/// dan yang tersisa hanya tugas tambahannya.
#[test]
fn dewan_guru_bertugas_tambahan_memenuhi_dewan_guru() {
    for r in ["dewan_guru_finance", "dewan_guru_absensi"] {
        assert!(role_satisfies(r, &["dewan_guru"]), "{r}");
        assert!(role_satisfies(r, &["admin", "dewan_guru"]), "{r}");
        assert!(role_satisfies(r, &[r]), "{r} cocok dengan namanya sendiri");
    }
}

/// Tapi TIDAK naik jadi admin. Mengurus uang atau mengesahkan absensi bukan
/// alasan untuk menata kelas, menunjuk wali, atau menyunting pengguna.
#[test]
fn dewan_guru_bertugas_tambahan_bukan_admin() {
    for r in ["dewan_guru_finance", "dewan_guru_absensi"] {
        assert!(!role_satisfies(r, &["admin"]), "{r} tak boleh setara admin");
        assert!(!role_satisfies(r, &["ketua"]), "{r}");
        assert!(!role_satisfies(r, &["santri"]), "{r}");
    }
}

/// Keduanya BERBEDA satu sama lain. Tugas tambahan yang satu bukan milik yang
/// lain — dan gerbang yang menyebut salah satunya tak boleh meloloskan keduanya.
#[test]
fn dua_tugas_tambahan_tak_saling_mencakup() {
    assert!(!role_satisfies("dewan_guru_absensi", &["dewan_guru_finance"]));
    assert!(!role_satisfies("dewan_guru_finance", &["dewan_guru_absensi"]));
    // Dewan guru biasa tak mendapat keduanya hanya karena namanya mirip.
    assert!(!role_satisfies("dewan_guru", &["dewan_guru_finance"]));
    assert!(!role_satisfies("dewan_guru", &["dewan_guru_absensi"]));
}

/// Peran barunya harus SAH di kolom `users.role`, kalau tidak ia bisa dipilih
/// di layar tapi ditolak database saat disimpan.
#[test]
fn peran_baru_sah_di_migrasi() {
    // Migrasi TERBARU yang menulis ulang CHECK — 95, bukan 93. CHECK ditulis
    // utuh tiap kali, jadi yang berlaku selalu yang terakhir.
    let m = std::fs::read_to_string("migration/95_peran_dewan_guru_sarpras.sql")
        .expect("migration/95 hilang");
    assert!(m.contains("'dewan_guru_finance'"), "peran tak ada di CHECK");
    assert!(m.contains("'dewan_guru_absensi'"), "peran tak ada di CHECK");
    // Peran lama tak boleh ikut hilang saat CHECK-nya ditulis ulang.
    for lama in ["'admin'", "'ketua'", "'dewan_guru'", "'santri'", "'santri_finance'",
                 "'parent'", "'penjaga'"] {
        assert!(m.contains(lama), "{lama} hilang dari CHECK — akun lama jadi tak sah");
    }
}

/// Peran dewan guru bertugas-tambahan diuji sebagai SATU KELUARGA.
///
/// Daftarnya ditulis sekali di sini lalu dipakai seluruh uji di bawah. Menambah
/// peran keempat berarti menambah SATU baris — dan kalau ada daftar di kode yang
/// terlewat, uji-uji itu langsung menyebut peran mana dan daftar mana.
const TUGAS_TAMBAHAN: &[&str] =
    &["dewan_guru_finance", "dewan_guru_absensi", "dewan_guru_sarpras"];

/// `dewan_guru_sarpras` (migrasi 95) mengurus sarana, dan HANYA itu.
///
/// Ia tak boleh diam-diam ikut memegang uang: mendata kursi patah dan melihat
/// siapa menunggak adalah dua kepercayaan yang berbeda.
#[test]
fn sarpras_mengurus_sarana_bukan_uang() {
    let api = std::fs::read_to_string("src/web/api.rs").expect("api.rs");

    let ambil = |nama: &str| -> Vec<String> {
        let blok = api
            .split(&format!("const {nama}: &[&str] = &["))
            .nth(1)
            .unwrap_or_else(|| panic!("{nama} tak ditemukan"));
        let blok = &blok[..blok.find("];").expect("penutup daftar")];
        blok.split('"').skip(1).step_by(2).map(|x| x.to_string()).collect()
    };

    let sarana = ambil("SARANA_MANAGE_ROLES");
    assert!(
        sarana.iter().any(|r| r == "dewan_guru_sarpras"),
        "sarpras harus boleh menata sarana: {sarana:?}"
    );

    for daftar in ["FINANCE_ROLES", "BILL_ADMIN_ROLES"] {
        let isi = ambil(daftar);
        assert!(
            !isi.iter().any(|r| r == "dewan_guru_sarpras"),
            "sarpras TIDAK boleh masuk {daftar}: {isi:?}"
        );
    }
}

/// Tugas tambahan tak saling mencakup — tiga peran, tiga kepercayaan terpisah.
#[test]
fn tugas_tambahan_saling_asing() {
    for a in TUGAS_TAMBAHAN {
        for b in TUGAS_TAMBAHAN {
            if a == b {
                continue;
            }
            assert!(!role_satisfies(a, &[b]), "{a} tak boleh memenuhi {b}");
        }
        // Semuanya dewan guru penuh, tapi tak satu pun naik jadi admin.
        assert!(role_satisfies(a, &["dewan_guru"]), "{a} harus dewan guru");
        assert!(!role_satisfies(a, &["admin"]), "{a} bukan admin");
        // Dan dewan guru polos tak mendapat tugas tambahan siapa pun.
        assert!(!role_satisfies("dewan_guru", &[a]), "dewan_guru bukan {a}");
    }
}

/// Seluruh keluarga hadir di SETIAP daftar peran yang ada di kode.
///
/// Inventarisnya lahir dari tiga laporan bug berturut-turut, masing-masing
/// karena satu daftar terlewat. Ditulis sebagai satu uji supaya peran berikutnya
/// tak perlu menemukannya lagi lewat cara yang sama.
#[test]
fn keluarga_tugas_tambahan_lengkap_di_semua_daftar() {
    let berkas: &[(&str, &str)] = &[
        ("VALID_ROLES", "src/repository/users.rs"),
        ("INVITABLE_ROLES", "src/service/registration.rs"),
        ("SEKALI_PAKAI_ROLES", "src/service/registration.rs"),
        ("STAFF_INVITABLE_ROLES", "src/models/auth.rs"),
        ("PERAN (manajemen_user)", "src/web/pages/manajemen_user.rs"),
        ("ROLES (kontrol_pengguna)", "src/web/pages/kontrol_pengguna.rs"),
        ("nav_for", "src/web/components.rs"),
        ("role_home/role_label/role_satisfies", "src/models/auth.rs"),
    ];
    for (nama, path) in berkas {
        let isi = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
        for r in TUGAS_TAMBAHAN {
            assert!(
                isi.contains(&format!("\"{r}\"")),
                "`{r}` tak disebut di {nama} ({path}) — peran itu akan gagal diam-diam di sana"
            );
        }
    }
}

/// `VALID_ROLES` di repository HARUS sepadan persis dengan CHECK di migrasi.
///
/// Ini pagar terakhir sebelum `UPDATE users SET role`, dan ia pernah tertinggal:
/// peran barunya sudah sah di database dan sudah muncul di semua dropdown, tapi
/// menyimpannya dijawab "Peran tidak valid atau pengguna tidak ditemukan" —
/// kalimat yang menuduh datanya, padahal daftarnya yang kurang.
///
/// Diadu DUA ARAH dengan sengaja. Peran yang ada di CHECK tapi tak di sini
/// mustahil diberikan; yang ada di sini tapi tak di CHECK lolos sampai database
/// lalu gagal dengan galat constraint mentah. Keduanya sunyi.
#[test]
fn valid_roles_sepadan_dengan_check_migrasi() {
    // Migrasi TERBARU yang menulis ulang CHECK. Ia ditulis utuh tiap kali, jadi
    // yang berlaku di database selalu yang terakhir — menunjuk migrasi lama di
    // sini membuat uji ini membandingkan dengan aturan yang sudah tak berlaku.
    let m = std::fs::read_to_string("migration/95_peran_dewan_guru_sarpras.sql")
        .expect("migration/95 hilang");

    // Ambil isi CHECK (role IN ( … )) apa adanya, lalu petik tiap literalnya.
    let dalam = m
        .split("CHECK (role IN (")
        .nth(1)
        .expect("CHECK (role IN ( … ) tak ditemukan di migrasi 95");
    let dalam = &dalam[..dalam.find("))").expect("penutup CHECK tak ditemukan")];
    let mut di_migrasi: Vec<String> = dalam
        .split('\'')
        .skip(1)
        .step_by(2)
        .map(|x| x.to_string())
        .collect();
    di_migrasi.sort();
    assert!(!di_migrasi.is_empty(), "parser gagal membaca daftar peran migrasi");

    let mut di_kode: Vec<String> =
        ppm::repository::VALID_ROLES.iter().map(|r| r.to_string()).collect();
    di_kode.sort();

    assert_eq!(
        di_kode, di_migrasi,
        "VALID_ROLES (repository/users.rs) tak sepadan dengan CHECK di migrasi 93.\n\
         Hanya di kode: {:?}\nHanya di migrasi: {:?}",
        di_kode.iter().filter(|r| !di_migrasi.contains(r)).collect::<Vec<_>>(),
        di_migrasi.iter().filter(|r| !di_kode.contains(r)).collect::<Vec<_>>(),
    );
}

/// Tiap peran yang bisa DIPILIH di layar harus bisa DISIMPAN.
///
/// Menutup celah yang menghasilkan laporan tadi dari sisi lain: dropdown yang
/// menawarkan peran yang ditolak `set_role` adalah janji yang tak ditepati, dan
/// orang baru mengetahuinya setelah menekan simpan.
#[test]
fn setiap_peran_yang_ditawarkan_layar_bisa_disimpan() {
    for (berkas, isi) in [
        ("manajemen_user.rs", include_str!("../src/web/pages/manajemen_user.rs")),
        ("kontrol_pengguna.rs", include_str!("../src/web/pages/kontrol_pengguna.rs")),
    ] {
        for r in ["dewan_guru_finance", "dewan_guru_absensi"] {
            if isi.contains(&format!("\"{r}\"")) {
                assert!(
                    ppm::repository::VALID_ROLES.contains(&r),
                    "{berkas} menawarkan `{r}` tapi VALID_ROLES menolaknya"
                );
            }
        }
    }
}

/// Peran baru bisa DIDAFTARKAN lewat undangan — TIGA daftar, semuanya wajib.
///
/// Ketiganya terpisah dan gampang lupa salah satu:
///   • `INVITABLE_ROLES`  (service) — server menolak "Peran tidak valid" tanpanya;
///   • `ROLES`            (layar kontrol pengguna) — tanpanya tak ada pilihannya;
///   • `STAFF_INVITABLE_ROLES` (models) — tanpanya `can_invite` menganggapnya
///     peran biasa, dan pagar "hanya admin" tak berlaku untuknya.
///
/// Uji ini lahir dari kejadian nyata: peran barunya sudah bisa DIBERIKAN dari
/// /manajemen-user tapi tak muncul sama sekali saat membuat user baru, karena
/// layar undangan memakai daftar peran SENDIRI.
#[test]
fn peran_baru_bisa_diundang_di_ketiga_daftar() {
    let reg = std::fs::read_to_string("src/service/registration.rs").expect("registration.rs");
    let layar =
        std::fs::read_to_string("src/web/pages/kontrol_pengguna.rs").expect("kontrol_pengguna.rs");

    for r in ["dewan_guru_finance", "dewan_guru_absensi"] {
        assert!(reg.contains(&format!("\"{r}\"")), "{r} tak ada di INVITABLE_ROLES");
        assert!(layar.contains(&format!("\"{r}\"")), "{r} tak ada di ROLES layar undangan");
        assert!(
            ppm::models::is_staff_invite(r),
            "{r} harus dianggap peran STAF — kalau tidak, bukan-admin boleh mencetak undangannya"
        );
    }
}

/// Hanya admin yang boleh mencetak undangan untuk peran barunya.
///
/// Undangan adalah TAUTAN: ia bisa diteruskan, disalin, dan dipakai orang yang
/// tak pernah dimaksud. Untuk peran yang memegang kunci keuangan atau
/// pengesahan kehadiran lintas kelas, siapa yang boleh mencetaknya adalah
/// keputusan pengurus — bukan efek samping dari seseorang yang kebetulan punya
/// tombol undangan.
#[test]
fn undangan_peran_baru_hanya_oleh_admin() {
    for r in ["dewan_guru_finance", "dewan_guru_absensi"] {
        assert!(ppm::models::can_invite("admin", r), "admin boleh: {r}");
        assert!(ppm::models::can_invite("ketua", r), "ketua mencakup admin: {r}");
        assert!(!ppm::models::can_invite("dewan_guru", r), "dewan guru TAK boleh: {r}");
        assert!(!ppm::models::can_invite("santri", r), "{r}");
    }
}

/// Peran berkunci tak boleh punya undangan berkuota banyak — satu tautan bocor
/// tak boleh menghasilkan sejumlah petugas keuangan.
#[test]
fn undangan_peran_berkunci_sekali_pakai() {
    let reg = std::fs::read_to_string("src/service/registration.rs").expect("registration.rs");
    assert!(reg.contains("SEKALI_PAKAI_ROLES"), "pagar kuota hilang");
    assert!(
        reg.contains("if SEKALI_PAKAI_ROLES.contains(&role) { 1 } else { 1000 }"),
        "kuota tak lagi dipaksa 1 untuk peran berkunci"
    );
    // Dipotong dari NAMA konstantanya, bukan dari tanda tangan lengkapnya.
    //
    // Versi pertama mencari `"SEKALI_PAKAI_ROLES: &[&str] = &["` sebagai satu
    // baris utuh — dan pecah begitu daftarnya bertambah panjang sehingga
    // rustfmt memindahkan `&[` ke baris berikutnya. Uji yang gagal karena
    // PEMFORMATAN, bukan karena aturannya berubah, adalah uji yang lama-lama
    // dimatikan orang.
    let blok = reg
        .split("pub const SEKALI_PAKAI_ROLES")
        .nth(1)
        .expect("SEKALI_PAKAI_ROLES tak ditemukan");
    let blok = &blok[..blok.find("];").unwrap_or(blok.len())];
    for r in TUGAS_TAMBAHAN {
        assert!(blok.contains(r), "{r} harus sekali pakai");
    }
}

/// Peran barunya bisa DIBERIKAN dari /manajemen-user. Tanpa baris di daftar
/// itu, perannya ada di database tapi tak ada layar yang bisa memasangnya —
/// persis yang sempat terjadi pada penjaga.
#[test]
fn peran_baru_bisa_dipilih_di_manajemen_user() {
    let src = std::fs::read_to_string("src/web/pages/manajemen_user.rs")
        .expect("manajemen_user.rs hilang");
    assert!(src.contains("\"dewan_guru_finance\""));
    assert!(src.contains("\"dewan_guru_absensi\""));
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
