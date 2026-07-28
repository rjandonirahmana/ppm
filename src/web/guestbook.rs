//! web/guestbook.rs — Check-in tamu dari mesin IoT (migrasi 35). Handler axum
//! multipart (di luar server-fn). Auth via `api_key` device (bukan cookie).
//!
//! POST /api/guestbook  multipart: `api_key`, `code` (6-digit), `file` (JPEG wajah)
//!   → cari kode di Redis (hapus) → simpan kunjungan + wajah (RustFS) →
//!     tandai done (HP tamu polling) → balas JSON { ok, name }.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::models::GuestCheckin;
use crate::state::AppState;

pub async fn checkin(
    Extension(state): Extension<Arc<AppState>>,
    _headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let mut api_key = String::new();
    let mut code = String::new();
    let mut file_bytes: Option<Vec<u8>> = None;

    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            "api_key" => api_key = field.text().await.unwrap_or_default(),
            "code" | "guest" => code = field.text().await.unwrap_or_default(),
            "file" | "image" => match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            },
            _ => {}
        }
    }

    let code = code.trim();
    if code.is_empty() {
        return (StatusCode::BAD_REQUEST, "code wajib diisi").into_response();
    }

    // Validasi device via api_key.
    let device = match crate::repository::find_device_by_key(&state.pool, api_key.trim()).await {
        Ok(Some(d)) => d,
        Ok(None) => return (StatusCode::UNAUTHORIZED, "api_key tidak dikenal").into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Cari + konsumsi kode di Redis.
    let mut redis = state.redis.clone();
    let guest = match crate::service::guest::consume_guest(&mut redis, code).await {
        Ok(Some(g)) => g,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, "Kode tidak ditemukan atau kedaluwarsa").into_response()
        }
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    };

    // Simpan wajah ke RustFS (kalau storage aktif & ada foto).
    let mut face_url: Option<String> = None;
    if let (Some(storage), Some(bytes)) = (state.storage.clone(), file_bytes) {
        if !bytes.is_empty() && bytes.len() <= 10_000_000 {
            let key = format!(
                "guests/{}-{}.jpg",
                chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
                code
            );
            match storage.upload_bytes(bytes, &key, "image/jpeg").await {
                Ok(u) => face_url = Some(u),
                Err(e) => {
                    crate::service::telegram::report_error(502, "Guest face upload", e.to_string());
                }
            }
        }
    }

    // Simpan baris buku tamu.
    if let Err(e) = crate::repository::insert_guest_visit(
        &state.pool,
        &guest.name,
        &guest.phone,
        &guest.purpose,
        face_url.as_deref(),
        Some(device.id),
    )
    .await
    {
        crate::service::telegram::report_error(500, "Guest insert", e.to_string());
        return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
    }

    // Tandai sukses agar HP tamu (polling /tamu) menampilkan ✅.
    let checkin = GuestCheckin {
        name: guest.name.clone(),
        face_url: face_url.clone().unwrap_or_default(),
    };
    let _ = crate::service::guest::mark_done(&mut redis, code, &checkin).await;

    (StatusCode::OK, Json(json!({ "ok": true, "name": guest.name }))).into_response()
}
