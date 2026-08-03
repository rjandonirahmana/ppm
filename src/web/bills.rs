//! web/bills.rs — Santri unggah bukti bayar tagihan (migrasi 37). Handler axum
//! multipart (di luar server-fn). Auth cookie; hanya boleh set bukti tagihan
//! MILIK SENDIRI (guard di repo::set_proof). Balas JSON `{ url }`.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::state::AppState;

pub async fn upload_proof(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };

    let Some(storage) = state.storage.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Penyimpanan (RustFS) belum aktif.").into_response();
    };

    let mut bill_id: i64 = 0;
    let mut file_bytes: Option<Vec<u8>> = None;
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            "bill_id" => {
                bill_id = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0);
            }
            "file" | "image" => match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            },
            _ => {}
        }
    }

    if bill_id <= 0 {
        return (StatusCode::BAD_REQUEST, "bill_id wajib.").into_response();
    }
    let Some(bytes) = file_bytes.filter(|b| !b.is_empty() && b.len() <= crate::web::limits::IMAGE_MAX) else {
        return (StatusCode::BAD_REQUEST, "File bukti tidak valid (maks 10MB).").into_response();
    };

    let key = format!(
        "bills/{}-{}.jpg",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        claims.user_id
    );
    let url = match storage.upload_bytes(bytes, &key, "image/jpeg").await {
        Ok(u) => u,
        Err(e) => {
            crate::service::telegram::report_error(502, "Bill proof upload", e.to_string());
            return (StatusCode::BAD_GATEWAY, format!("Upload gagal: {e}")).into_response();
        }
    };

    // Guard kepemilikan di query (user_id = pengunggah).
    match crate::repository::set_proof(&state.pool, bill_id, claims.user_id, &url).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "url": url }))).into_response(),
        Ok(false) => (StatusCode::FORBIDDEN, "Bukan tagihan Anda.").into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}
