//! models/materials.rs — Payload "Materials Library" (migrasi 17): file
//! bersama diunggah staf (murattal/kitab/video), lepas dari rekaman sesi.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialItem {
    pub id: i64,
    pub title: String,
    /// "audio" | "document" | "video" | "link".
    pub kind: String,
    pub file_url: String,
    /// "MP3 • 24.5 MB • 12 Okt 2025" (kosong bagian yang tak relevan, mis. link).
    pub meta_label: String,
}
