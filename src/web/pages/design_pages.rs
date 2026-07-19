//! web/pages/design_pages.rs — Halaman hasil PORT desain (statis, visual).
//!
//! Tiap komponen menyuntik HTML desain via `include_str!` + `inner_html`.
//! Tailwind (Play CDN di shell) menata kelasnya.
//!
//! Dua macro:
//!   • `raw_page_mobile!` — halaman berorientasi PONSEL → dibungkus <DeviceFrame>
//!     (desktop: ponsel terpusat di atas latar gradient, mirip login).
//!   • `raw_page!`        — halaman ADMIN dgn sidebar desktop → full-width apa adanya.
//! Data masih statis; disambung server functions saat backend siap.

use leptos::prelude::*;
use leptos_meta::Title;

use crate::web::components::DeviceFrame;

/// Halaman admin (punya layout desktop sendiri: sidebar) → full-width.
macro_rules! raw_page {
    ($name:ident, $file:literal, $title:literal) => {
        #[component]
        pub fn $name() -> impl IntoView {
            view! {
                <Title text=$title />
                <div inner_html=include_str!(concat!("html/", $file))></div>
            }
        }
    };
}

/// Halaman mobile → dibungkus bingkai device (bagus di desktop, seperti login).
macro_rules! raw_page_mobile {
    ($name:ident, $file:literal, $title:literal) => {
        #[component]
        pub fn $name() -> impl IntoView {
            view! {
                <Title text=$title />
                <DeviceFrame>
                    <div inner_html=include_str!(concat!("html/", $file))></div>
                </DeviceFrame>
            }
        }
    };
}

// ── Admin (sidebar desktop, full-width) ────────────────────────────────────────
// staf/guru/dewan_guru/poin/poin_dewan: sudah jadi halaman DINAMIS (data ASLI) —
// lihat staf.rs, analisis.rs, poin.rs. Versi HTML statis dihapus dari sini.
raw_page!(KoneksiOrtuPage, "koneksi_ortu.html", "Koneksi Orang Tua — PPM AFM");
// verifikasi_tahap2: kini halaman DINAMIS (verifikasi_tahap2.rs, data DB).

// ── Mobile (device frame di desktop) ───────────────────────────────────────────
// portal_santri/riwayat/profil: sudah jadi halaman DINAMIS (izin.rs, riwayat.rs,
// profil.rs — data DB). Versi embed dihapus.
// orang_tua: sudah jadi halaman DINAMIS (ortu_beranda.rs dkk, data DB).
// verifikasi_pamong: sudah jadi halaman DINAMIS (verifikasi_pamong.rs, data DB).
raw_page_mobile!(HalaqahDaftarPage, "halaqah_daftar.html", "Daftar Halaqah — PPM AFM");
raw_page_mobile!(HalaqahMulaiPage, "halaqah_mulai.html", "Mulai Sesi Halaqah — PPM AFM");
raw_page_mobile!(HalaqahLivePage, "halaqah_live.html", "Sesi Halaqah Live — PPM AFM");
raw_page_mobile!(RekamanPage, "rekaman.html", "Rekaman Materi — PPM AFM");
