//! models/gallery.rs — Foto kegiatan (galeri beranda, migrasi 34). Shared
//! (SSR + hydrate) karena jadi tipe balikan server fn.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityPhoto {
    pub id: i64,
    pub url: String,
    pub caption: String,
    pub sort_order: i32,
}
