//! device_api.rs — Handler HTTP perangkat RFID gerbang (server-only, lapisan tipis).
//!
//! POST /api/rfid/scan  { "api_key": "...", "card": 1234567890 } — gerbang KELAS
//!   (hadir/terlambat per jadwal). Logika di `service::attendance::record_scan`.
//! POST /api/rfid/gate  { "api_key": "...", "card": 1234567890 } — gerbang UTAMA
//!   pondok (toggle masuk/keluar). Logika di `service::gate::record_gate_scan`.
//! Device SAMA (rfid_devices/api_key); dua fisik unit berbeda cukup pukul URL
//! berbeda — tak perlu tabel/kolom device baru.

use std::sync::Arc;

use axum::{http::StatusCode, Extension, Json};

use crate::models::{GateScanResponse, RfidScanRequest, RfidScanResponse};
use crate::service::attendance::{record_scan, ScanError};
use crate::service::gate::record_gate_scan;
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

fn fail_gate(code: StatusCode, message: impl Into<String>) -> (StatusCode, Json<GateScanResponse>) {
    (
        code,
        Json(GateScanResponse { ok: false, message: message.into(), student: None, direction: None }),
    )
}

pub async fn rfid_scan(
    Extension(state): Extension<Arc<AppState>>,
    Json(req): Json<RfidScanRequest>,
) -> (StatusCode, Json<RfidScanResponse>) {
    let mut redis = state.redis.clone();
    match record_scan(&state.pool, &mut redis, &req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err(ScanError::BadApiKey) => fail(StatusCode::UNAUTHORIZED, "api_key tidak dikenal"),
        Err(ScanError::UnknownCard) => fail(StatusCode::NOT_FOUND, "kartu tidak terdaftar"),
        Err(ScanError::Db(e)) => {
            crate::service::telegram::report_error(500, "RFID scan", e.to_string());
            fail(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    }
}

/// POST /api/rfid/gate — gerbang utama pondok (masuk/keluar toggle otomatis).
pub async fn rfid_gate(
    Extension(state): Extension<Arc<AppState>>,
    Json(req): Json<RfidScanRequest>,
) -> (StatusCode, Json<GateScanResponse>) {
    match record_gate_scan(&state.pool, &req).await {
        Ok(resp) => (StatusCode::OK, Json(resp)),
        Err(ScanError::BadApiKey) => fail_gate(StatusCode::UNAUTHORIZED, "api_key tidak dikenal"),
        Err(ScanError::UnknownCard) => fail_gate(StatusCode::NOT_FOUND, "kartu tidak terdaftar"),
        Err(ScanError::Db(e)) => {
            crate::service::telegram::report_error(500, "RFID gate", e.to_string());
            fail_gate(StatusCode::INTERNAL_SERVER_ERROR, e.to_string())
        }
    }
}
