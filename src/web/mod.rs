//! web — App universal Leptos (SSR + hydrate): shell HTML, router, halaman.

pub mod api;
pub mod app;
pub mod components;
// Siaran suara sesi: handler axum (server) + komponen klien (universal).
#[cfg(feature = "ssr")]
pub mod live_audio;
pub mod live_audio_ui;
// SSE ruang live (server): sinyal perubahan → klien refetch.
#[cfg(feature = "ssr")]
pub mod live_events;
pub mod pages;
