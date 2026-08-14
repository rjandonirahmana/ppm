//! tests/navigasi_ui.rs — Setiap `<a href="/…">` harus benar-benar bisa dituju.
//!
//! ── GEJALA YANG MELAHIRKAN TES INI ───────────────────────────────────────────
//! "Tombol ditekan, halaman tak pindah. Setelah di-refresh baru pindah."
//!
//! Penyebabnya bukan hidrasi yang gagal, melainkan cara `leptos_router` bekerja:
//! ia memasang SATU pendengar klik di `window` dan mencegat setiap `<a>` yang
//! se-origin. Ia hanya melepaskannya ke peramban bila tautan itu punya salah
//! satu dari `download`, `rel="external"`, `target`, atau origin berbeda
//! (lihat `handle_anchor_click` di leptos_router/src/location/mod.rs).
//!
//! Akibatnya ada DUA cara sebuah tautan gagal secara diam-diam:
//!
//!   1. Menunjuk jalur yang BUKAN rute SPA (mis. endpoint Axum `/api/…`) tanpa
//!      penanda. Router mendorong URL-nya, tak ada rute yang cocok, dan yang
//!      muncul halaman fallback. Menyegarkan halaman "memperbaikinya" — karena
//!      muat-ulang sungguhan tak lewat router — dan itulah yang membuat gejalanya
//!      terasa acak.
//!   2. Salah ketik jalur rute SPA. Sama persis akibatnya.
//!
//! Keduanya lolos kompilator: `href` cuma string.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

fn baca(rel: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("gagal membaca {}: {e}", p.display()))
}

fn berkas_ui() -> Vec<String> {
    let akar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/web");
    let mut out = Vec::new();
    let mut antre = vec![akar];
    while let Some(dir) = antre.pop() {
        let Ok(baca) = fs::read_dir(&dir) else { continue };
        for e in baca.flatten() {
            let p = e.path();
            if p.is_dir() {
                antre.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p.to_string_lossy().to_string());
            }
        }
    }
    out.sort();
    out
}

/// Semua rute SPA yang dideklarasikan `<Route path=path!("…")>`.
fn rute_spa() -> Vec<String> {
    let src = baca("src/web/app.rs");
    let mut out = Vec::new();
    let mut sisa = src.as_str();
    while let Some(i) = sisa.find("path!(\"") {
        let s = &sisa[i + "path!(\"".len()..];
        let Some(j) = s.find('"') else { break };
        out.push(s[..j].to_string());
        sisa = &s[j..];
    }
    assert!(out.len() > 30, "hanya {} rute terbaca — parser rusak?", out.len());
    out
}

/// Apakah `href` cocok dengan salah satu rute (segmen `:param` cocok apa pun)?
fn cocok_rute(href: &str, rute: &[String]) -> bool {
    let bersih = href.split(['?', '#']).next().unwrap_or(href);
    let seg: Vec<&str> = bersih.split('/').filter(|s| !s.is_empty()).collect();
    rute.iter().any(|r| {
        let rs: Vec<&str> = r.split('/').filter(|s| !s.is_empty()).collect();
        rs.len() == seg.len() && rs.iter().zip(&seg).all(|(a, b)| a.starts_with(':') || a == b)
    })
}

/// Satu tautan: jalurnya, berkasnya, dan apakah ia ditandai "biar peramban saja".
struct Tautan {
    href: String,
    berkas: String,
    baris: usize,
    dilepas: bool,
}

/// Kumpulkan `href="/…"` MILIK ELEMEN `<a>` saja, beserta atribut sekitarnya.
///
/// Pembatasan ke `<a>` itu penting: `<link rel="icon" href="/icons/…">` dan
/// `<Stylesheet href="/pkg/ppm.css">` juga punya `href`, tapi tak satu pun
/// pernah diklik — router tak menyentuhnya, dan menuduhnya hanya melahirkan
/// alarm palsu. Versi pertama tes ini melaporkan enam.
///
/// Jendela 6 baris sesudah `href` cukup untuk atributnya: elemen `view!` selalu
/// ditulis berdempetan, dan jendela lebih lebar mulai memungut atribut milik
/// elemen berikutnya.
fn tautan() -> Vec<Tautan> {
    let mut out = Vec::new();
    for f in berkas_ui() {
        let src = baca(f.trim_start_matches(&format!("{}/", env!("CARGO_MANIFEST_DIR"))));
        let baris: Vec<&str> = src.lines().collect();
        for (i, l) in baris.iter().enumerate() {
            for pola in ["href=\"", "href=format!(\""] {
                let Some(k) = l.find(pola) else { continue };
                let s = &l[k + pola.len()..];
                let akhir = s.find(['"', '{']).unwrap_or(s.len());
                let href = &s[..akhir];
                if !href.starts_with('/') || href.starts_with("//") {
                    continue;
                }
                // Tag pembuka terdekat ke belakang harus `<a` — bukan `<link`,
                // `<Stylesheet`, atau `<Link`.
                let awal = i.saturating_sub(8);
                let sebelum = baris[awal..=i].join(" ");
                let Some(t) = sebelum.rfind('<') else { continue };
                let tag: String = sebelum[t + 1..]
                    .chars()
                    .take_while(|c| c.is_alphanumeric())
                    .collect();
                if tag != "a" {
                    continue;
                }

                let jendela = baris[i..(i + 6).min(baris.len())].join(" ");
                out.push(Tautan {
                    href: href.to_string(),
                    berkas: f.rsplit('/').next().unwrap_or(&f).to_string(),
                    baris: i + 1,
                    dilepas: jendela.contains("download")
                        || jendela.contains("rel=\"external\"")
                        || jendela.contains("target="),
                });
            }
        }
    }
    assert!(out.len() > 40, "hanya {} tautan terbaca — parser rusak?", out.len());
    out
}

/// Jalur yang memang BUKAN rute SPA dan dilayani Axum langsung. Berada di sini
/// berarti tautannya WAJIB ditandai `download`/`rel="external"`/`target`.
const NON_SPA: &[&str] = &["/api/", "/healthz", "/pkg/", "/fonts/"];

#[test]
fn tautan_non_spa_ditandai_agar_router_melepasnya() {
    let mut salah = Vec::new();
    for t in tautan() {
        if !NON_SPA.iter().any(|p| t.href.starts_with(p)) {
            continue;
        }
        if !t.dilepas {
            salah.push(format!(
                "{}:{} → {}  (tambahkan `download` atau `rel=\"external\"`)",
                t.berkas, t.baris, t.href
            ));
        }
    }
    assert!(
        salah.is_empty(),
        "tautan ke endpoint non-SPA yang akan DICEGAT router ({}):\n  {}\n\n\
         Gejalanya: klik tak melakukan apa pun (router mendorong URL lalu jatuh \
         ke fallback), tapi menyegarkan halaman berhasil.",
        salah.len(),
        salah.join("\n  ")
    );
}

#[test]
fn setiap_tautan_spa_menunjuk_rute_yang_ada() {
    let rute = rute_spa();
    let mut salah: BTreeMap<String, String> = BTreeMap::new();
    for t in tautan() {
        if NON_SPA.iter().any(|p| t.href.starts_with(p)) || t.dilepas {
            continue;
        }
        if !cocok_rute(&t.href, &rute) {
            salah.insert(t.href.clone(), format!("{}:{}", t.berkas, t.baris));
        }
    }
    assert!(
        salah.is_empty(),
        "tautan ke jalur yang tak punya rute ({}):\n  {}",
        salah.len(),
        salah
            .iter()
            .map(|(h, f)| format!("{h}  ← {f}"))
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

// ── Tes untuk parsernya sendiri ──────────────────────────────────────────────

#[test]
fn parser_membaca_rute_dan_tautan() {
    let rute = rute_spa();
    assert!(rute.iter().any(|r| r == "/"), "rute beranda tak terbaca");
    assert!(rute.iter().any(|r| r.contains(':')), "rute berparameter tak terbaca");
    assert!(tautan().len() > 40);
}

#[test]
fn pencocok_rute_menghormati_parameter() {
    let rute = vec!["/poin/:id".to_string(), "/kelas".to_string()];
    assert!(cocok_rute("/poin/7", &rute));
    assert!(cocok_rute("/kelas", &rute));
    assert!(cocok_rute("/kelas?tab=sesi", &rute), "query string tak boleh menggagalkan");
    assert!(!cocok_rute("/poin/7/detail", &rute), "jumlah segmen beda");
    assert!(!cocok_rute("/tidak-ada", &rute));
}

/// Justru kasus yang baru saja jadi bug: tautan unduhan TANPA penanda harus
/// terdeteksi, dan yang SUDAH ditandai harus dibiarkan.
#[test]
fn penanda_pelepas_dikenali() {
    let ada = tautan();
    let ekspor: Vec<&Tautan> = ada.iter().filter(|t| t.href.starts_with("/api/export")).collect();
    assert!(!ekspor.is_empty(), "tautan ekspor tak terbaca — parser rusak?");
    for t in ekspor {
        assert!(
            t.dilepas,
            "{}:{} → {} belum ditandai; router akan mencegatnya",
            t.berkas, t.baris, t.href
        );
    }
}
