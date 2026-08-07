//! lib.rs — Entry point unified SSR + Hydration untuk AFM SMART.
//!
//! Satu `App` universal: server render HTML (SSR), klien memasang reaktivitas
//! (hydrate) ke DOM yang sama → zero mismatch.

#![recursion_limit = "512"]

// DTO/struct bersama seluruh lapisan — dikompilasi untuk SEMUA target.
pub mod models;

// Modul web (App universal) — dikompilasi untuk SSR & Hydrate.
#[cfg(any(feature = "ssr", feature = "hydrate"))]
pub mod web;

// Modul server-only — tidak dikompilasi untuk WASM.
#[cfg(not(target_arch = "wasm32"))]
pub mod auth;
#[cfg(not(target_arch = "wasm32"))]
pub mod config;
#[cfg(not(target_arch = "wasm32"))]
pub mod device_api;
#[cfg(not(target_arch = "wasm32"))]
pub mod repository;
#[cfg(not(target_arch = "wasm32"))]
pub mod service;
#[cfg(not(target_arch = "wasm32"))]
pub mod state;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use web::app::App;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
