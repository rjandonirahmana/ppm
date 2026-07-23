//! service/materials.rs — "Materials Library" (migrasi 17): daftar + tambah
//! link + hapus. Upload FILE (audio/document/video) ditangani handler axum
//! murni `web/materials.rs` (butuh multipart, di luar server-fn) yang
//! memanggil `repository::materials` langsung setelah upload ke RustFS —
//! pola sama `web/live_audio.rs`.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use super::fmt::wib;
use crate::models::MaterialItem;
use crate::repository as repo;

fn fmt_size(bytes: Option<i64>) -> String {
    match bytes {
        Some(b) if b >= 1_000_000 => format!("{:.1} MB", b as f64 / 1_000_000.0),
        Some(b) if b > 0 => format!("{:.1} KB", b as f64 / 1_000.0),
        _ => String::new(),
    }
}

fn kind_ext_label(kind: &str) -> &'static str {
    match kind {
        "audio" => "MP3",
        "document" => "PDF",
        "video" => "Video",
        _ => "Link",
    }
}

pub async fn list_materials(pool: &Pool, limit: i64) -> Result<Vec<MaterialItem>> {
    Ok(repo::recent_materials(pool, limit)
        .await?
        .into_iter()
        .map(|m| {
            let date = super::fmt::fmt_date(m.created_at.with_timezone(&wib()).date_naive());
            let size = fmt_size(m.file_size);
            let meta_label = if m.kind == "link" {
                format!("Link • {date}")
            } else if size.is_empty() {
                format!("{} • {date}", kind_ext_label(&m.kind))
            } else {
                format!("{} • {size} • {date}", kind_ext_label(&m.kind))
            };
            MaterialItem { id: m.id, title: m.title, kind: m.kind, file_url: m.file_url, meta_label }
        })
        .collect())
}

/// Tambah materi berupa TAUTAN (mis. video YouTube) — tanpa upload file.
pub async fn add_link(pool: &Pool, uploaded_by: i64, title: &str, url: &str) -> Result<i64> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Judul materi wajib diisi.");
    }
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail!("Tautan harus diawali http:// atau https://");
    }
    repo::insert_material(pool, None, uploaded_by, title, "link", url, None, None).await
}

pub async fn delete_material(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_material(pool, id).await? {
        bail!("Materi tidak ditemukan.");
    }
    Ok(())
}
