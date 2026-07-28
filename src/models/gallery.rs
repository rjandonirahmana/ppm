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

/// Status check-in tamu (buku tamu, migrasi 35) — dipolling halaman /tamu untuk
/// menampilkan ✅ setelah mesin IoT mengonfirmasi kode + wajah.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuestCheckin {
    pub name: String,
    pub face_url: String,
}
