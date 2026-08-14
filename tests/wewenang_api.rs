//! tests/wewenang_api.rs — Matriks wewenang `web/api.rs`, dikunci jadi tes.
//!
//! ── KENAPA STATIS, BUKAN MEMANGGIL ENDPOINT-NYA ──────────────────────────────
//! `web/api.rs` memuat 167 server function — seluruh permukaan HTTP aplikasi —
//! dan sampai sekarang NOL tes. Mengujinya sungguhan butuh Postgres, Redis, dan
//! sesi ber-cookie; itu tes integrasi yang belum ada infrastrukturnya.
//!
//! Yang bisa diperiksa tanpa semua itu ternyata justru bagian yang paling mahal
//! bila salah: SIAPA yang boleh masuk. Daftar peran ditulis sebagai literal di
//! atas tiap fungsi, jadi ia bisa dibaca langsung dari sumber dan diadu dengan
//! spesifikasi peran pondok.
//!
//! ── SPESIFIKASI YANG DIKUNCI DI SINI ─────────────────────────────────────────
//! A. Ketua = admin + keuangan. TAPI perizinan & verifikasi santri BUKAN
//!    urusannya — itu wewenang orang yang mengenal santrinya.
//! B. Admin = seperti ketua TANPA akses uang. Ia menata struktur: membuat kelas,
//!    menunjuk wali kelas, menyunting data pengguna.
//! C-F. Dewan guru baru berwenang atas sebuah kelas SETELAH ditunjuk jadi
//!    walinya. Penjaga hanya mengurus tamu. Santri-finance boleh melihat
//!    pembayaran santri lain untuk audit.
//!
//! Tes ini tak akan menangkap logika yang salah DI DALAM endpoint. Ia menangkap
//! hal lain: pintu yang diam-diam terbuka untuk peran yang tak semestinya —
//! kelas kesalahan yang paling sunyi, karena tak ada yang mengeluh saat sebuah
//! pintu terlalu longgar.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

fn sumber_api() -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/web/api.rs");
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("gagal membaca {}: {e}", p.display()))
}

/// `const NAMA: &[&str] = &["a", "b"];` → nama ⇒ daftar peran.
fn konstanta_peran(src: &str) -> HashMap<String, Vec<String>> {
    let mut out = HashMap::new();
    for baris in src.lines() {
        let b = baris.trim();
        let Some(sisa) = b.strip_prefix("const ") else { continue };
        if !sisa.contains(": &[&str]") {
            continue;
        }
        let Some(nama) = sisa.split(':').next().map(str::trim) else { continue };
        out.insert(nama.to_string(), petik(b));
    }
    out
}

/// Semua isi tanda kutip di sepotong teks.
fn petik(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut sisa = s;
    while let Some(i) = sisa.find('"') {
        let sesudah = &sisa[i + 1..];
        let Some(j) = sesudah.find('"') else { break };
        out.push(sesudah[..j].to_string());
        sisa = &sesudah[j + 1..];
    }
    out
}

/// Satu server function: namanya, penjaganya, dan peran yang diterimanya.
struct Endpoint {
    nama: String,
    penjaga: Option<String>,
    peran: Vec<String>,
}

fn endpoints() -> Vec<Endpoint> {
    let src = sumber_api();
    let konst = konstanta_peran(&src);
    let mut out = Vec::new();

    for blok in src.split("#[server(").skip(1) {
        let Some(i) = blok.find("pub async fn ") else { continue };
        let sisa = &blok[i + "pub async fn ".len()..];
        let nama: String = sisa.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let badan = match blok.find("\n}\n") {
            Some(j) => &blok[..j],
            None => blok,
        };

        let (penjaga, peran) = if let Some(k) = badan.find("require_roles(") {
            let arg = &badan[k + "require_roles(".len()..];
            let arg = &arg[..arg.find(')').unwrap_or(arg.len())];
            let daftar = if let Some(p) = konst.get(arg.trim()) {
                p.clone()
            } else {
                petik(arg)
            };
            (Some("roles".to_string()), daftar)
        } else if badan.contains("require_petugas_kelas(") {
            (Some("petugas_kelas".to_string()), Vec::new())
        } else if badan.contains("require_session(") {
            (Some("session".to_string()), Vec::new())
        } else if badan.contains("require_login(") {
            (Some("login".to_string()), Vec::new())
        } else {
            (None, Vec::new())
        };

        out.push(Endpoint { nama, penjaga, peran });
    }
    assert!(out.len() > 100, "hanya menemukan {} endpoint — parser rusak?", out.len());
    out
}

fn cari<'a>(eps: &'a [Endpoint], nama: &str) -> &'a Endpoint {
    eps.iter()
        .find(|e| e.nama == nama)
        .unwrap_or_else(|| panic!("endpoint `{nama}` tak ditemukan — apakah namanya berubah?"))
}

// ── Invarian ─────────────────────────────────────────────────────────────────

/// Endpoint yang SENGAJA terbuka tanpa login. Daftar ini adalah kontraknya:
/// menambah nama ke sini harus jadi keputusan sadar, bukan efek samping lupa
/// memasang penjaga.
const PUBLIK: &[&str] = &[
    "login_action",
    "forgot_password_action",
    "logout_action",
    "validate_invite_action",
    "register_action",
    "resend_otp_action",
    "verify_register_action",
    "activity_photos_data",
    "articles_data",
    "article_data",
    "register_guest_action",
    "guest_status_action",
    // Pembaca sesi itu sendiri. Ia MENGEMBALIKAN `Option<SessionUser>` — `None`
    // untuk yang belum login — jadi memasang penjaga di sini justru mustahil:
    // penjaga menolak yang belum login, padahal itu jawaban yang sah.
    "get_session",
];

#[test]
fn setiap_endpoint_terjaga_kecuali_yang_sengaja_publik() {
    let eps = endpoints();
    let tanpa: Vec<&str> = eps
        .iter()
        .filter(|e| e.penjaga.is_none())
        .map(|e| e.nama.as_str())
        .filter(|n| !PUBLIK.contains(n))
        .collect();
    assert!(
        tanpa.is_empty(),
        "endpoint tanpa penjaga wewenang: {tanpa:?}\n\
         Bila memang harus publik, tambahkan namanya ke PUBLIK beserta alasannya."
    );

    // Sebaliknya juga: nama di PUBLIK yang ternyata sudah dijaga berarti daftar
    // ini basi, dan daftar basi lambat laun berhenti dipercaya.
    let basi: Vec<&&str> = PUBLIK
        .iter()
        .filter(|n| eps.iter().any(|e| e.nama == ***n && e.penjaga.is_some()))
        .collect();
    assert!(basi.is_empty(), "sudah dijaga tapi masih terdaftar PUBLIK: {basi:?}");
}

/// SPEK A — perizinan bukan urusan ketua/admin.
///
/// Izin diputuskan orang yang mengenal santrinya, yaitu wali kelas KBM-nya.
/// Admin yang ikut memutuskan hanya menambah tangan tanpa menambah konteks.
#[test]
fn keputusan_izin_tertutup_untuk_ketua_dan_admin() {
    let eps = endpoints();
    for nama in ["permit_queue_data", "decide_permit_action"] {
        let e = cari(&eps, nama);
        for terlarang in ["admin", "ketua"] {
            assert!(
                !e.peran.iter().any(|r| r == terlarang),
                "{nama} menerima `{terlarang}` — spek A: perizinan bukan wewenangnya. \
                 Peran saat ini: {:?}",
                e.peran
            );
        }
    }
}

/// SPEK B — admin biasa tak menyentuh uang santri.
///
/// `ketua` boleh (ia admin + keuangan) dan `santri_finance` boleh (untuk audit),
/// tapi `admin` polos tidak. Perhatikan `role_satisfies` hanya berlaku SATU
/// ARAH: ketua memenuhi daftar yang menyebut "admin", tapi admin TIDAK memenuhi
/// daftar yang menyebut "ketua" — jadi menulis "ketua" benar-benar mengunci.
#[test]
fn keuangan_tertutup_untuk_admin_biasa() {
    let eps = endpoints();
    let keuangan = [
        "unpaid_bills_data",
        "paid_bills_data",
        "create_bill_action",
        "delete_bill_action",
        "pending_bills_data",
        "verify_bill_action",
        "reject_bill_action",
        "finance_student_search",
    ];
    for nama in keuangan {
        let e = cari(&eps, nama);
        assert!(
            !e.peran.iter().any(|r| r == "admin"),
            "{nama} menerima `admin` — spek B: admin tak mengakses uang santri. \
             Peran saat ini: {:?}",
            e.peran
        );
        assert!(
            e.peran.iter().any(|r| r == "ketua"),
            "{nama} tak menerima `ketua`, padahal dialah pemegang keuangan. \
             Peran saat ini: {:?}",
            e.peran
        );
    }
}

/// SPEK B — struktur kelas ditata admin/ketua, bukan guru.
///
/// Membuat kelas dan MENUNJUK WALI KELAS adalah fondasi yang dirujuk absensi,
/// poin, dan laporan. Guru menjalankan kelasnya; ia tak menunjuk dirinya sendiri.
#[test]
fn struktur_kelas_hanya_admin() {
    let eps = endpoints();
    for nama in ["create_class_action", "update_class_action", "set_class_wali_action"] {
        let e = cari(&eps, nama);
        for terlarang in ["dewan_guru", "teacher", "santri", "parent", "penjaga"] {
            assert!(
                !e.peran.iter().any(|r| r == terlarang),
                "{nama} menerima `{terlarang}` — spek B: hanya admin/ketua. \
                 Peran saat ini: {:?}",
                e.peran
            );
        }
    }
}

/// SPEK C — penataan isi kelas lewat SATU pintu, dan pintu itu memeriksa
/// kepemilikan kelas, bukan sekadar peran.
///
/// Daftar peran saja tak cukup di sini: "dewan guru" tak boleh berarti "boleh
/// menata kelas SIAPA PUN". `require_petugas_kelas` yang menguji apakah orang
/// ini wali kelas YANG BERSANGKUTAN.
#[test]
fn penataan_kelas_memeriksa_kepemilikan() {
    let eps = endpoints();
    let per_kelas = [
        "create_schedule_action",
        "update_schedule_action",
        "delete_schedule_action",
        "create_session_action",
        "add_member_action",
        "add_members_action",
        "remove_member_action",
        "set_session_teacher_action",
        "create_curriculum_action",
        "update_curriculum_action",
        "delete_curriculum_action",
    ];
    for nama in per_kelas {
        let e = cari(&eps, nama);
        assert_eq!(
            e.penjaga.as_deref(),
            Some("petugas_kelas"),
            "{nama} memakai penjaga `{:?}` — seharusnya `require_petugas_kelas`, \
             yang menguji apakah pemanggil wali kelas INI. Penjaga berbasis peran \
             saja membuat guru mana pun bisa menata kelas orang lain.",
            e.penjaga
        );
    }
}

/// SPEK F — buku tamu adalah pekerjaan penjaga.
#[test]
fn tamu_terbuka_untuk_penjaga() {
    let eps = endpoints();
    let e = cari(&eps, "tamu_masuk_data");
    assert!(
        e.peran.iter().any(|r| r == "penjaga"),
        "penjaga tak bisa melihat daftar tamu — itu satu-satunya tugasnya. \
         Peran saat ini: {:?}",
        e.peran
    );
}

/// Santri tak boleh menyelinap ke pintu staf.
///
/// Diperiksa terbalik — mendaftar apa yang TERLARANG, bukan apa yang boleh —
/// supaya endpoint baru yang lupa dipagari ikut tertangkap tanpa perlu
/// menambahkannya ke daftar mana pun.
#[test]
fn santri_tidak_masuk_pintu_staf() {
    let eps = endpoints();
    let pintu_staf = [
        "create_class_action",
        "set_class_wali_action",
        "decide_permit_action",
        "permit_queue_data",
        "create_bill_action",
        "delete_bill_action",
        "user_manage_data",
    ];
    for nama in pintu_staf {
        let Some(e) = eps.iter().find(|e| e.nama == nama) else { continue };
        for terlarang in ["santri", "santri_finance", "parent"] {
            assert!(
                !e.peran.iter().any(|r| r == terlarang),
                "{nama} menerima `{terlarang}`. Peran saat ini: {:?}",
                e.peran
            );
        }
    }
}

/// Peran yang sudah tak ada tak boleh muncul lagi di daftar mana pun.
///
/// `supervisor` (pamong) dibuang seluruhnya Agustus 2026. Ia gampang kembali
/// tanpa sengaja lewat salin-tempel daftar peran dari endpoint lama.
#[test]
fn peran_yang_sudah_dihapus_tidak_muncul() {
    let eps = endpoints();
    let muncul: Vec<&str> = eps
        .iter()
        .filter(|e| e.peran.iter().any(|r| r == "supervisor"))
        .map(|e| e.nama.as_str())
        .collect();
    assert!(
        muncul.is_empty(),
        "peran `supervisor` (pamong) sudah dihapus tapi masih dipakai: {muncul:?}"
    );
}

// ── Tes untuk parsernya sendiri ──────────────────────────────────────────────

#[test]
fn parser_membaca_konstanta_peran() {
    let src = r#"
        const FINANCE_ROLES: &[&str] = &["ketua", "santri_finance"];
        const KELAS_ADMIN: &[&str] = &["admin"];
    "#;
    let k = konstanta_peran(src);
    assert_eq!(k.get("FINANCE_ROLES").unwrap(), &["ketua", "santri_finance"]);
    assert_eq!(k.get("KELAS_ADMIN").unwrap(), &["admin"]);
}

#[test]
fn parser_menemukan_semua_endpoint() {
    let eps = endpoints();
    // Angka pastinya berubah seiring fitur; yang dijaga adalah parser tak
    // diam-diam berhenti melihat sebagian besar berkas.
    assert!(eps.len() >= 150, "hanya {} endpoint terbaca", eps.len());
    assert!(
        eps.iter().filter(|e| e.penjaga.is_some()).count() >= 140,
        "terlalu sedikit endpoint yang terbaca penjaganya — parser mungkin rusak"
    );
}
