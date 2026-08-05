//! web — App universal Leptos (SSR + hydrate): shell HTML, router, halaman.

// Upload foto kegiatan galeri (server, multipart — di luar server-fn).
#[cfg(feature = "ssr")]
pub mod activity_photos;
// Check-in tamu dari mesin IoT (server, multipart — di luar server-fn).
#[cfg(feature = "ssr")]
pub mod guestbook;
// Unggah bukti bayar tagihan (server, multipart — di luar server-fn).
#[cfg(feature = "ssr")]
pub mod bills;
pub mod api;
pub mod app;
pub mod components;
// Deteksi tipe berkas dari isinya — dipakai SEMUA jalur unggah.
#[cfg(feature = "ssr")]
pub mod filetype;
// Unduh laporan PDF/Excel (server, axum — di luar server-fn).
#[cfg(feature = "ssr")]
pub mod export;
// Batas ukuran unggahan — dipakai bersama oleh router (main.rs) & handler.
#[cfg(feature = "ssr")]
pub mod limits;
// Siaran suara sesi: handler axum (server) + komponen klien (universal).
#[cfg(feature = "ssr")]
pub mod live_audio;
pub mod live_audio_ui;
// SSE ruang live (server): sinyal perubahan → klien refetch.
#[cfg(feature = "ssr")]
pub mod live_events;
// Upload file Materials Library (server, multipart — di luar server-fn).
#[cfg(feature = "ssr")]
pub mod materials;
pub mod pages;
