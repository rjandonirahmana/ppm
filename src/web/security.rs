//! web/security.rs — Header keamanan HTTP + Content-Security-Policy.
//! SATU sumber kebenaran, sama seperti [`crate::web::limits`] untuk batas unggah.
//!
//! ── KENAPA DIBAGI DUA TEMPAT ─────────────────────────────────────────────────
//! Kebijakannya tak bisa seluruhnya diletakkan di satu tempat, dan itu bukan
//! pilihan gaya:
//!
//!   • CSP butuh NONCE, dan nonce baru lahir SAAT halaman dirender (Leptos
//!     membuatnya per-permintaan lewat `provide_nonce`). Lapisan tower di
//!     `main.rs` berjalan di luar render dan tak mungkin mengetahuinya — maka
//!     CSP dikirim sebagai `<meta http-equiv>` di dalam `<head>`, ditulis oleh
//!     [`csp_dengan_nonce`].
//!
//!   • `frame-ancestors` dan `report-uri` DIABAIKAN browser bila datang lewat
//!     `<meta>` — spesifikasinya menyebutkan itu secara eksplisit. Jadi
//!     perlindungan clickjacking-nya dikirim sebagai header sungguhan
//!     (`X-Frame-Options`), bersama header lain yang tak butuh nonce.
//!
//! Menaruh keduanya di berkas ini membuat ketidaksepakatan di antara mereka
//! kelihatan saat dibaca — persis alasan `limits.rs` ada.
//!
//! ── KENAPA `<meta>` HARUS PALING ATAS DI `<head>` ────────────────────────────
//! CSP lewat `<meta>` hanya berlaku untuk apa pun yang datang SESUDAHNYA. Kalau
//! ia dipasang lewat `leptos_meta::Meta`, tempat munculnya ditentukan
//! `<MetaTags/>` — yang di shell proyek ini berada JAUH DI BAWAH dua `<script>`
//! inline. Skripnya lolos tanpa diperiksa, dan kebijakannya jadi hiasan. Karena
//! itu `<meta>`-nya ditulis langsung sebagai elemen pertama `<head>`.

// ── Header statis (tak butuh nonce) ──────────────────────────────────────────

/// Header yang dipasang pada SEMUA respons, apa pun rutenya.
///
/// `X-Frame-Options: DENY` menggantikan `frame-ancestors` yang tak bisa lewat
/// `<meta>` (lihat catatan modul). Aplikasi ini tak pernah dipasang di dalam
/// iframe mana pun, jadi DENY — bukan SAMEORIGIN — adalah jawaban yang benar.
///
/// `Referrer-Policy` sengaja `strict-origin-when-cross-origin`, bukan
/// `no-referrer`: banyak jalur di aplikasi ini memuat id di URL (`/sesi/123`,
/// `/kelas/45`), dan mengirimkannya ke situs luar lewat header Referer
/// membocorkan struktur data pesantren tanpa ada yang memintanya. Nilai ini
/// menahan jalur+query pada permintaan lintas-origin tapi tetap mengirim origin
/// saja, sehingga CDN font & gambar tetap bekerja.
///
/// `microphone=(self)` WAJIB ADA: siaran suara sesi (`web/live_audio_ui.rs`)
/// memanggil `getUserMedia`. Menulis `microphone=()` seperti izin lain akan
/// mematikan fitur itu diam-diam — tanpa galat yang menyebut CSP.
pub const HEADER_STATIS: &[(&str, &str)] = &[
    ("x-content-type-options", "nosniff"),
    ("referrer-policy", "strict-origin-when-cross-origin"),
    ("x-frame-options", "DENY"),
    ("cross-origin-opener-policy", "same-origin"),
    (
        "permissions-policy",
        "camera=(), geolocation=(), payment=(), usb=(), microphone=(self)",
    ),
];

/// HSTS — hanya masuk akal bila situsnya memang dilayani lewat HTTPS, jadi
/// dipasang HANYA saat `LEPTOS_ENV=PROD` (patokan yang sama dipakai flag
/// `Secure` pada cookie sesi, lihat `web::api::set_auth_cookie`).
///
/// TANPA `includeSubDomains` dan TANPA `preload`, dan itu disengaja. Keduanya
/// sulit ditarik kembali: `includeSubDomains` memaksa HTTPS pada SELURUH
/// subdomain — termasuk yang dikelola orang lain dan mungkin masih HTTP — dan
/// `preload` menanamkannya ke dalam browser itu sendiri, di mana pencabutannya
/// makan waktu berbulan-bulan. Satu tahun pada satu host sudah memberi hampir
/// seluruh manfaatnya tanpa satu pun risiko itu.
pub const HSTS: (&str, &str) = ("strict-transport-security", "max-age=31536000");

/// Apakah aplikasi sedang berjalan sebagai produksi.
pub fn produksi() -> bool {
    std::env::var("LEPTOS_ENV").as_deref() == Ok("PROD")
}

// ── Content-Security-Policy ──────────────────────────────────────────────────

/// Bagian CSP yang tak bergantung pada nonce. Dipisah agar bisa diuji sendiri.
///
/// Catatan per direktif — masing-masing dibuka selebar yang BENAR-BENAR dipakai,
/// tidak lebih:
///
/// • `script-src` — `'self'` DIPERTAHANKAN dan `'strict-dynamic'` sengaja TIDAK
///   dipakai. `'strict-dynamic'` membuat browser mengabaikan `'self'`, dan
///   `<link rel="modulepreload">` yang disisipkan `HydrationScripts` bukan
///   skrip yang "dimuat oleh skrip tepercaya" — ia permintaan tersendiri, jadi
///   ia akan diblokir. Yang hilang cuma pramuat, tapi konsolnya dipenuhi galat
///   CSP di setiap halaman, dan galat CSP yang selalu ada adalah galat CSP yang
///   tak pernah dibaca. `'wasm-unsafe-eval'` wajib: tanpa itu `.wasm` hidrasi
///   tak boleh dikompilasi, dan aplikasinya ter-render tapi mati.
///
/// • `style-src` — `'unsafe-inline'` masih diperlukan: ada puluhan atribut
///   `style="…"` di komponen (mis. `components.rs` untuk sheet & backdrop).
///   Atribut gaya TIDAK bisa diberi nonce oleh mekanisme apa pun — nonce hanya
///   berlaku untuk elemen `<style>`. Nonce juga sengaja TIDAK ditambahkan ke
///   direktif ini: begitu sebuah nonce hadir di `style-src`, browser MENGABAIKAN
///   `'unsafe-inline'`, dan seluruh atribut gaya itu mati sekaligus.
///
/// • `img-src`/`media-src` — `https:` karena foto & rekaman disajikan dari
///   RustFS di host yang ditentukan `RUSTFS_PUBLIC_URL` saat jalan. Hostnya
///   tak diketahui saat kompilasi, jadi menuliskannya di sini akan salah pada
///   pemasangan mana pun yang mengubahnya. `blob:` untuk pemutaran MediaSource
///   siaran suara; `data:` untuk gambar sebaris kecil.
///
/// • `connect-src` — `'self'` untuk server fn & SSE ruang live.
///
/// • `frame-ancestors` TIDAK ADA DI SINI. Ia diabaikan lewat `<meta>`; lihat
///   `X-Frame-Options` di [`HEADER_STATIS`].
pub const CSP_TANPA_NONCE: &str = "default-src 'self'; \
     base-uri 'self'; \
     object-src 'none'; \
     form-action 'self'; \
     style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; \
     font-src 'self' https://fonts.gstatic.com; \
     img-src 'self' data: blob: https:; \
     media-src 'self' blob: https:; \
     connect-src 'self'";

/// CSP lengkap untuk satu permintaan, dengan nonce yang dipakai `<script>`
/// inline dan skrip hidrasi Leptos.
///
/// `nonce` `None` berarti kita tidak sedang merender di server (atau fitur
/// `nonce` mati). Di situ CSP dikembalikan KOSONG, bukan tanpa nonce: kebijakan
/// yang menyebut `script-src 'self'` tanpa nonce akan memblokir persis skrip
/// kita sendiri, dan halaman menjadi mati total. Lebih baik tak ada kebijakan
/// daripada kebijakan yang memblokir aplikasinya sendiri.
pub fn csp_dengan_nonce(nonce: Option<&str>) -> String {
    match nonce {
        Some(n) => format!("{CSP_TANPA_NONCE}; script-src 'self' 'nonce-{n}' 'wasm-unsafe-eval'"),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `frame-ancestors` diabaikan browser bila dikirim lewat `<meta>`. Kalau
    /// suatu saat ia dituliskan di sini, perlindungan clickjacking-nya berhenti
    /// bekerja TANPA gejala apa pun — dan `X-Frame-Options` mungkin ikut
    /// dibuang karena terlihat mubazir.
    #[test]
    fn csp_meta_tak_memuat_frame_ancestors() {
        assert!(
            !CSP_TANPA_NONCE.contains("frame-ancestors"),
            "frame-ancestors tak berlaku lewat <meta> — pakai X-Frame-Options"
        );
        assert!(
            HEADER_STATIS.iter().any(|(k, _)| *k == "x-frame-options"),
            "X-Frame-Options wajib ada selama CSP dikirim lewat <meta>"
        );
    }

    /// Tanpa `'wasm-unsafe-eval'` bundel hidrasi tak boleh dikompilasi: halaman
    /// tetap tampil (HTML SSR) tapi tak pernah bisa diklik — gejala yang persis
    /// sama dengan bug bundel 22 MB yang sudah pernah dikejar proyek ini.
    #[test]
    fn script_src_mengizinkan_wasm() {
        let csp = csp_dengan_nonce(Some("abc123"));
        assert!(csp.contains("'wasm-unsafe-eval'"), "{csp}");
        assert!(csp.contains("'nonce-abc123'"), "{csp}");
        assert!(csp.contains("script-src 'self'"), "{csp}");
    }

    /// Nonce di `style-src` akan MEMATIKAN `'unsafe-inline'` di sana (aturan
    /// CSP3), dan setiap atribut `style="…"` di komponen ikut mati.
    #[test]
    fn style_src_tak_pernah_diberi_nonce() {
        let csp = csp_dengan_nonce(Some("abc123"));
        let style = csp
            .split("; ")
            .find(|d| d.trim_start().starts_with("style-src"))
            .expect("style-src harus ada");
        assert!(!style.contains("nonce-"), "style-src tak boleh bernonce: {style}");
        assert!(style.contains("'unsafe-inline'"), "{style}");
    }

    /// Siaran suara memanggil `getUserMedia`. `microphone=()` akan mematikannya
    /// tanpa galat yang menyebut-nyebut CSP.
    #[test]
    fn mikrofon_tetap_diizinkan() {
        let pp = HEADER_STATIS
            .iter()
            .find(|(k, _)| *k == "permissions-policy")
            .expect("permissions-policy harus ada")
            .1;
        assert!(pp.contains("microphone=(self)"), "{pp}");
    }

    /// Tanpa nonce, kebijakan apa pun akan memblokir skrip kita sendiri.
    #[test]
    fn tanpa_nonce_csp_kosong() {
        assert!(csp_dengan_nonce(None).is_empty());
    }

    /// HSTS yang sulit ditarik kembali tak boleh menyelinap masuk.
    #[test]
    fn hsts_tak_memaksa_subdomain_atau_preload() {
        assert!(!HSTS.1.contains("includeSubDomains"), "{}", HSTS.1);
        assert!(!HSTS.1.contains("preload"), "{}", HSTS.1);
    }
}
