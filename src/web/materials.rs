//! web/materials.rs — Upload FILE ke Materials Library (migrasi 17), handler
//! axum murni (di luar server-fn — butuh multipart, sama alasan
//! web/live_audio.rs). Auth cookie manual (admin/dewan_guru saja).
//!
//! `kind='link'` (tautan, mis. YouTube) TIDAK lewat sini — server fn biasa
//! `add_material_link_action` (web/api.rs) sudah cukup, tanpa file.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;

use crate::state::AppState;

/// `role_satisfies`, BUKAN `matches!` harfiah.
///
/// Daftar harfiah tak mengenal peran yang MENCAKUPI `dewan_guru` —
/// `dewan_guru_finance` dan `dewan_guru_absensi` (migrasi 93), dan sebelumnya
/// `teacher`. Semuanya dewan guru sepenuhnya, tapi di daftar harfiah mereka
/// ditolak diam-diam: tak ada galat, layarnya hanya kosong atau tombolnya tak
/// pernah muncul.
///
/// Itu persis kesalahan yang catatan `role_satisfies` sendiri peringatkan —
/// "cara lama membuat delapan endpoint lupa menulisnya" — dan ia terulang di
/// tujuh tempat begitu dua peran baru ditambahkan. Satu pintu, satu aturan.
fn is_manager(role: &str) -> bool {
    role == "admin" || crate::models::role_satisfies(role, &["dewan_guru"])
}

/// Ekstensi file → (kind, tipe isi yang DITERIMA) untuk Materials Library.
///
/// Tipe isinya JAMAK, bukan tunggal, karena satu ekstensi bisa sah membungkus
/// beberapa tanda tangan. `.mp4` dan `.mov` memakai wadah ISO-BMFF yang sama —
/// berkas `.mp4` ber-major-brand `qt  ` (lazim dari ponsel dan konverter) sah
/// sepenuhnya, tapi dengan satu tipe saja ia ditolak "isi tak cocok dengan
/// ekstensinya", padahal ia diputar normal di mana-mana.
///
/// Yang dipakai sebagai label simpan adalah tipe hasil SNIFF bila ia termasuk
/// daftar ini — supaya labelnya menggambarkan isi sebenarnya, bukan ekstensinya.
fn classify(filename: &str) -> Option<(&'static str, &'static [&'static str])> {
    // Berkas tanpa titik: `rsplit` mengembalikan seluruh namanya, dan itu tak
    // akan cocok dengan ekstensi mana pun — jadi ia jatuh ke `None`, benar.
    let ext = filename.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "mp3" => ("audio", &["audio/mpeg"][..]),
        "wav" => ("audio", &["audio/wav"][..]),
        "ogg" | "opus" => ("audio", &["audio/ogg"][..]),
        // Rekaman suara dari ponsel: wadah ISO-BMFF berisi audio saja.
        "m4a" => ("audio", &["audio/mp4"][..]),
        "pdf" => ("document", &["application/pdf"][..]),
        "mp4" | "mov" | "m4v" => ("video", &["video/mp4", "video/quicktime"][..]),
        "webm" | "mkv" => ("video", &["video/webm"][..]),
        _ => return None,
    })
}

/// Ubah judul jadi potongan nama berkas yang AMAN untuk kunci objek.
///
/// ── KENAPA TIDAK CUKUP "ganti yang bukan alfanumerik jadi strip" ────────────
/// `char::is_alphanumeric` sadar-Unicode, jadi huruf Arab, aksara lain, dan
/// huruf beraksen LOLOS apa adanya ke dalam kunci objek. Di pesantren judul
/// materi berhuruf Arab bukan hal langka, dan kunci berisi byte non-ASCII harus
/// dikodekan dengan benar di setiap lapisan yang menyentuhnya — sesuatu yang
/// gagal diam-diam, bukan dengan pesan yang menuntun.
///
/// Yang juga diperbaiki di sini: kunci tak lagi bisa membengkak mengikuti judul
/// sepanjang apa pun, strip beruntun diringkas jadi satu, dan judul yang
/// SELURUHNYA di luar ASCII tak lagi menghasilkan nama kosong — ia jatuh ke
/// "materi", yang tetap bisa dibaca manusia saat menelusuri isi bucket.
fn slug(title: &str) -> String {
    const MAKS: usize = 60;
    let mut out = String::with_capacity(MAKS);
    let mut strip_terakhir = false;
    for c in title.chars() {
        // ASCII saja — sengaja lebih sempit dari `is_alphanumeric`.
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            strip_terakhir = false;
        } else if !strip_terakhir && !out.is_empty() {
            out.push('-');
            strip_terakhir = true;
        }
        if out.len() >= MAKS {
            break;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "materi".to_string()
    } else {
        out
    }
}

/// POST /api/materials/upload — multipart: `title`, opsional `class_id`, `file`.
pub async fn upload(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if !is_manager(&claims.role) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(storage) = state.storage.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Penyimpanan file (RustFS) belum dikonfigurasi di server. Gunakan opsi Tautan, atau hubungi admin teknis.",
        )
            .into_response();
    };

    let mut title = String::new();
    let mut class_id: Option<i64> = None;
    // Berkasnya ada di DISK, bukan di RAM: materi bisa 100 MB (rekaman kajian,
    // pdf kitab), dan menahannya di memori sampai PUT ke RustFS selesai adalah
    // cara paling cepat menghabiskan RAM VPS. Lihat `web::multipart`.
    let mut berkas: Option<crate::web::multipart::BerkasSementara> = None;
    let mut filename = String::new();

    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            "title" => title = field.text().await.unwrap_or_default(),
            "class_id" => {
                let s = field.text().await.unwrap_or_default();
                class_id = s.trim().parse::<i64>().ok().filter(|v| *v > 0);
            }
            "file" => {
                filename = field.file_name().unwrap_or_default().to_string();
                match crate::web::multipart::terima_berkas(
                    field,
                    crate::web::limits::MATERIAL_MAX,
                    "maks 100MB",
                )
                .await
                {
                    Ok(b) => berkas = Some(b),
                    Err(resp) => return resp,
                }
            }
            _ => {}
        }
    }

    // Diperiksa SEBELUM berkasnya dikirim ke penyimpanan objek. Kolomnya
    // `VARCHAR(200)`; judul yang lebih panjang dulu baru ketahuan di `INSERT`,
    // sesudah berkasnya terlanjur terunggah — pengunggah melihat galat Postgres
    // mentah, dan berkasnya tertinggal yatim di bucket.
    let title = match crate::service::materials::periksa_judul(&title) {
        Ok(t) => t,
        Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
    };
    let Some(berkas) = berkas else {
        return (StatusCode::BAD_REQUEST, "File wajib diunggah.").into_response();
    };
    // Batas ATAS sudah dijaga saat menulis (request diputus di tengah aliran);
    // yang tersisa diperiksa di sini hanya berkas kosong.
    if berkas.ukuran == 0 {
        return (StatusCode::BAD_REQUEST, "Ukuran file tidak valid (maks 100MB).").into_response();
    }
    let Some((kind, diterima)) = classify(&filename) else {
        return (
            StatusCode::BAD_REQUEST,
            "Jenis file tidak didukung (gunakan mp3/m4a/wav/ogg, pdf, atau mp4/mov/webm).",
        )
            .into_response();
    };
    // Ekstensi hanya nama pilihan pengunggah — isinya yang menentukan. Tanpa cek
    // ini, berkas apa pun bisa dinamai `.pdf` lalu tersimpan berlabel
    // `application/pdf` dan disajikan kembali dengan label itu. Yang dibaca
    // cukup byte pertamanya, yang memang ditahan `terima_berkas` di memori.
    // Label yang disimpan diambil dari ISI, bukan dari ekstensi — selama isinya
    // termasuk yang sah untuk ekstensi itu. `.mp4` ber-brand QuickTime karena
    // itu tersimpan sebagai `video/quicktime`, yang memang isinya.
    let Some(content_type) = berkas.tipe_isi().filter(|t| diterima.contains(t)) else {
        return (
            StatusCode::BAD_REQUEST,
            "Isi file tidak cocok dengan ekstensinya.",
        )
            .into_response();
    };

    let ext = crate::web::filetype::ext_for(content_type);
    // Prefix `ppm/` dibuang: bucket sudah "ppm" (dulu jadi `/ppm/ppm/materials/...`).
    let key = format!("materials/{}-{}.{}", slug(title), chrono::Utc::now().timestamp(), ext);

    let size = berkas.ukuran as i64;
    // Streaming dari disk — berkasnya tak pernah dimuat utuh ke RAM, baik saat
    // diterima maupun saat diteruskan.
    let url = match storage.upload_file(berkas.path(), &key, content_type).await {
        Ok(u) => u,
        Err(e) => {
            crate::service::telegram::report_error(502, "Materials upload", e.to_string());
            return (StatusCode::BAD_GATEWAY, format!("Upload gagal: {e}")).into_response();
        }
    };

    match crate::repository::insert_material(
        &state.pool, class_id, claims.user_id, title, kind, &url, Some(content_type), Some(size),
    )
    .await
    {
        Ok(id) => (StatusCode::OK, id.to_string()).into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Materials insert", e.to_string());
            // Berkasnya sudah terlanjur ada di penyimpanan objek sementara
            // barisnya tidak — tanpa dibuang, ia tak akan pernah terjangkau
            // siapa pun lagi dan tetap memakan ruang selamanya. Best-effort:
            // kegagalan membuangnya tak boleh menutupi galat aslinya.
            storage.delete_by_url_best_effort(&url).await;
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── slug(): judul → potongan kunci objek ─────────────────────────────────

    /// Judul biasa: huruf kecil, spasi & tanda baca jadi satu strip.
    #[test]
    fn slug_judul_biasa() {
        assert_eq!(slug("Kitab Ta'lim Muta'allim"), "kitab-ta-lim-muta-allim");
        assert_eq!(slug("Bab 3 — Adab Murid"), "bab-3-adab-murid");
    }

    /// Strip beruntun diringkas jadi satu. Tanpa ini "Bab 3  —  Adab"
    /// menghasilkan "bab-3-----adab", nama yang tak terbaca siapa pun.
    #[test]
    fn slug_tak_menumpuk_strip() {
        assert_eq!(slug("Bab   3  ---  Adab"), "bab-3-adab");
        assert!(!slug("a !!! b").contains("--"));
    }

    /// Tak boleh berawalan atau berakhiran strip.
    #[test]
    fn slug_tak_berstrip_di_ujung() {
        let s = slug("### Materi Baru ###");
        assert_eq!(s, "materi-baru");
        assert!(!s.starts_with('-') && !s.ends_with('-'));
    }

    /// WORST CASE — judul yang seluruhnya di luar ASCII.
    ///
    /// Huruf Arab bukan hal langka untuk judul materi di pesantren, dan
    /// `char::is_alphanumeric` yang dipakai versi lama MELOLOSKANNYA apa adanya
    /// ke dalam kunci objek. Sekarang ia disaring, dan judul yang habis tersaring
    /// jatuh ke nama cadangan — bukan string kosong yang menghasilkan kunci
    /// berawalan "-".
    #[test]
    fn slug_judul_non_ascii_jatuh_ke_cadangan() {
        assert_eq!(slug("تعليم المتعلم"), "materi");
        assert_eq!(slug("日本語"), "materi");
        assert_eq!(slug("🙂🙂🙂"), "materi");
    }

    /// Huruf ASCII di antara huruf non-ASCII tetap dipertahankan.
    #[test]
    fn slug_menyaring_non_ascii_tapi_menyimpan_sisanya() {
        assert_eq!(slug("Bab 1 — تعليم"), "bab-1");
        assert_eq!(slug("Café Adab"), "caf-adab");
    }

    /// WORST CASE — judul kosong atau hanya spasi.
    #[test]
    fn slug_judul_kosong_jatuh_ke_cadangan() {
        assert_eq!(slug(""), "materi");
        assert_eq!(slug("   "), "materi");
        assert_eq!(slug("!@#$%^&*()"), "materi");
    }

    /// WORST CASE — judul yang mencoba keluar dari direktori.
    ///
    /// Judul ikut menyusun kunci objek. Kalau garis miring dan titik lolos, satu
    /// judul bisa menulis ke jalur lain di bucket. Yang menahannya adalah
    /// penyaring ASCII-alfanumerik di sini, dan uji ini yang menjaganya tetap
    /// begitu.
    #[test]
    fn slug_tak_bisa_keluar_direktori() {
        for jahat in [
            "../../etc/passwd",
            r"..\..\windows\system32",
            "a/b/c",
            "....//....//x",
        ] {
            let s = slug(jahat);
            assert!(!s.contains('/'), "{jahat} → {s}");
            assert!(!s.contains('\\'), "{jahat} → {s}");
            assert!(!s.contains('.'), "{jahat} → {s}");
        }
    }

    /// WORST CASE — judul sangat panjang tak boleh membengkakkan kunci objek.
    #[test]
    fn slug_dibatasi_panjangnya() {
        let panjang = "a".repeat(5_000);
        assert!(slug(&panjang).len() <= 60, "{}", slug(&panjang).len());

        let kalimat = "Bab Tentang Adab Murid Kepada Gurunya ".repeat(50);
        let s = slug(&kalimat);
        assert!(s.len() <= 60);
        assert!(!s.ends_with('-'));
    }

    /// Judul yang HANYA huruf besar tetap turun jadi huruf kecil.
    #[test]
    fn slug_selalu_huruf_kecil() {
        assert_eq!(slug("MATERI BARU"), "materi-baru");
    }

    /// Judul dengan baris baru & tab — datang dari tempel-salin.
    #[test]
    fn slug_menangani_spasi_putih_apa_pun() {
        assert_eq!(slug("Bab\n1\tAdab"), "bab-1-adab");
    }

    // ── classify(): ekstensi → jenis & tipe isi yang diterima ────────────────

    /// WORST CASE — `.mp4` ber-brand QuickTime.
    ///
    /// Wadahnya sama persis dengan MP4 dan berkasnya diputar normal di mana
    /// pun, tapi dengan satu tipe yang diterima ia ditolak "isi tak cocok
    /// dengan ekstensinya". Sekarang kedua tipe diterima untuk ekstensi itu.
    #[test]
    fn mp4_menerima_brand_quicktime() {
        let (_, diterima) = classify("kajian.mp4").expect("mp4 didukung");
        assert!(diterima.contains(&"video/mp4"));
        assert!(diterima.contains(&"video/quicktime"));
    }

    /// `.mov` dari iPhone sekarang diterima — dulu berhenti di "jenis file
    /// tidak didukung" padahal isinya sudah bisa dikenali.
    #[test]
    fn mov_dan_m4v_didukung() {
        assert!(classify("rekaman.mov").is_some());
        assert!(classify("rekaman.m4v").is_some());
    }

    /// Rekaman suara ponsel (.m4a) diterima sebagai AUDIO.
    #[test]
    fn m4a_didukung_sebagai_audio() {
        let (kind, diterima) = classify("kajian.m4a").expect("m4a didukung");
        assert_eq!(kind, "audio");
        assert_eq!(diterima, &["audio/mp4"]);
    }

    /// Ekstensi huruf besar — pengunggah dari Windows sering begini.
    #[test]
    fn ekstensi_huruf_besar_diterima() {
        assert!(classify("KITAB.PDF").is_some());
        assert!(classify("Kajian.Mp3").is_some());
    }

    /// WORST CASE — nama berkas aneh yang tak boleh membuat panik.
    #[test]
    fn nama_berkas_aneh_tak_memanik() {
        assert!(classify("").is_none());
        assert!(classify("tanpatitik").is_none());
        assert!(classify("berakhir.titik.").is_none());
        assert!(classify(".pdf").is_some(), "berkas tersembunyi berekstensi sah");
        assert!(classify("arsip.tar.gz").is_none());
        assert!(classify("laporan.pdf.exe").is_none(), "ekstensi ganda menipu");
        assert!(classify("....").is_none());
        assert!(classify("تعليم.pdf").is_some(), "nama non-ASCII, ekstensi sah");
    }

    /// Jenis yang memang tak diterima tetap ditolak — pelonggaran di atas tak
    /// boleh membuka pintu untuk berkas yang bisa dieksekusi atau dirender.
    #[test]
    fn jenis_berbahaya_tetap_ditolak() {
        for nama in ["a.exe", "a.sh", "a.html", "a.svg", "a.js", "a.php", "a.zip", "a.docx"] {
            assert!(classify(nama).is_none(), "{nama} tak boleh diterima");
        }
    }
}
