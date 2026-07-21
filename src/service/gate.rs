//! service/gate.rs — Scan RFID gerbang UTAMA pondok (masuk/keluar santri),
//! TERPISAH dari gerbang kelas (service::attendance::record_scan). Device sama
//! `rfid_devices` (api_key), endpoint & tabel log beda (repository::gate).

use anyhow::Result;
use deadpool_postgres::Pool;

use crate::models::{GateScanResponse, RfidScanRequest};
use crate::repository as repo;
use crate::service::attendance::ScanError;

/// Toggle status gerbang: scan pertama = keluar, scan berikutnya = masuk, dst.
/// Firmware TIDAK perlu tahu arah — cukup kirim api_key+card sama persis
/// dengan gerbang kelas.
pub async fn record_gate_scan(pool: &Pool, req: &RfidScanRequest) -> Result<GateScanResponse, ScanError> {
    let Some(device) = repo::find_device_by_key(pool, &req.api_key).await? else {
        return Err(ScanError::BadApiKey);
    };
    let Some((user_id, name)) = repo::find_user_by_card(pool, req.card).await? else {
        return Err(ScanError::UnknownCard);
    };

    let direction = repo::toggle_gate(pool, user_id, Some(device.id)).await?;
    tracing::info!(user_id, card = req.card, direction, "gerbang pondok: scan tercatat");

    let message = if direction == "out" { "santri keluar pondok" } else { "santri masuk pondok" };
    Ok(GateScanResponse {
        ok: true,
        message: message.into(),
        student: Some(name),
        direction: Some(direction),
    })
}
