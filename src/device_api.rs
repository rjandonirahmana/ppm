//! device_api.rs — Handler HTTP perangkat RFID gerbang (server-only, lapisan tipis).
//!
//! POST /api/rfid/scan  { "api_key": "...", "card": 1234567890 }
//! Logika di `service::attendance::record_scan`.

use std::sync::Arc;

use axum::{http::StatusCode, Extension, Json};

use crate::models::{RfidScanRequest, RfidScanResponse};
use crate::service::attendance::{record_scan, ScanError};
use crate::state::AppState;

fn fail(code: StatusCode, message: impl Into<String>) -> (StatusCode, Json<RfidScanResponse>) {
    (
        code,
        Json(RfidScanResponse {
            ok: false,
            message: message.into(),
            student: None,
            status: None,
        }),
    )
}

pub async fn rfid_scan(
    Extension(state): Extension<Arc<AppState>>,
    Json(req): Json<RfidScanRequest>,
) -> (StatusCode, Json<RfidScanResponse>) {
    match record_scan(&state.pool, &req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err(ScanError::BadApiKey) => fail(StatusCode::UNAUTHORIZED, "api_key tidak dikenal"),
        Err(ScanError::UnknownCard) => fail(StatusCode::NOT_FOUND, "kartu tidak terdaftar"),
        Err(ScanError::Db(e)) => fail(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
    }
}
