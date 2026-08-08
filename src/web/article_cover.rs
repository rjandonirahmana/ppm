//! web/article_cover.rs — Unggah gambar sampul artikel (migrasi 69).
//! Handler axum murni (multipart, di luar server-fn — sama alasan materials.rs).
//!
//! Terpisah dari unggahan galeri karena hasilnya berbeda: yang ini TIDAK
//! menambah baris apa pun, hanya menaruh berkas di penyimpanan objek lalu
//! mengembalikan URL-nya untuk disimpan di kolom `articles.cover_url`. Kalau
//! dilewatkan ke jalur galeri, tiap sampul artikel akan ikut muncul di grid
//! foto kegiatan halaman depan.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::state::AppState;

fn is_manager(role: &str) -> bool {
    matches!(role, "admin" | "ketua")
}

fn classify_image(filename: &str) -> Option<&'static str> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => return None,
    })
}

/// POST /api/articles/cover — multipart dengan satu field `file`.
/// Balas `{ "url": "…" }`.
pub async fn upload(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if !is_manager(&claims.role) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(storage) = state.storage.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Penyimpanan file (RustFS) belum dikonfigurasi di server.",
        )
            .into_response();
    };

    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = String::new();
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        if field.name().unwrap_or_default() == "file" {
            filename = field.file_name().unwrap_or_default().to_string();
            match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            }
        }
    }

    let Some(bytes) = file_bytes else {
        return (StatusCode::BAD_REQUEST, "File wajib diunggah.").into_response();
    };
    if bytes.is_empty() || bytes.len() > crate::web::limits::IMAGE_MAX {
        return (StatusCode::BAD_REQUEST, "Ukuran gambar tidak valid (maks 10MB).")
            .into_response();
    }
    let Some(content_type) = classify_image(&filename) else {
        return (
            StatusCode::BAD_REQUEST,
            "Jenis file tidak didukung (gunakan jpg/png/webp/gif).",
        )
            .into_response();
    };
    // Ekstensi cuma nama yang dipilih pengunggah; isinya yang menentukan.
    if !crate::web::filetype::matches(&bytes, content_type) {
        return (
            StatusCode::BAD_REQUEST,
            "Isi file tidak cocok dengan ekstensinya — pastikan ini benar-benar gambar.",
        )
            .into_response();
    }

    let key = format!(
        "artikel/{}-{}.{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        claims.user_id,
        crate::web::filetype::ext_for(content_type)
    );
    match storage.upload_bytes(bytes, &key, content_type).await {
        Ok(url) => (StatusCode::OK, Json(json!({ "url": url }))).into_response(),
        Err(e) => {
            crate::service::telegram::report_error(502, "Article cover upload", e.to_string());
            (StatusCode::BAD_GATEWAY, format!("Upload gagal: {e}")).into_response()
        }
    }
}
