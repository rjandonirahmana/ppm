//! service/attendance.rs — Alur absensi: scan RFID + verifikasi pamong.

use anyhow::Result;
use chrono::Utc;
use deadpool_postgres::Pool;

use super::fmt::{fmt_when, wib};
use crate::models::{PamongData, PendingAtt, RfidScanRequest, RfidScanResponse};
use crate::repository as repo;

/// Error scan dibedakan agar handler bisa memberi kode HTTP yang tepat.
pub enum ScanError {
    BadApiKey,
    UnknownCard,
    Db(anyhow::Error),
}

impl From<anyhow::Error> for ScanError {
    fn from(e: anyhow::Error) -> Self {
        ScanError::Db(e)
    }
}

/// Proses satu scan kartu dari perangkat gerbang:
/// device → santri → jadwal aktif → present/late → simpan (dedup per hari).
/// Scan di luar jadwal tetap dicatat sebagai log gerbang (schedule NULL).
pub async fn record_scan(pool: &Pool, req: &RfidScanRequest) -> Result<RfidScanResponse, ScanError> {
    let Some(device) = repo::find_device_by_key(pool, &req.api_key).await? else {
        return Err(ScanError::BadApiKey);
    };
    let gate = device.location.unwrap_or(device.device_name);

    let Some((user_id, name)) = repo::find_user_by_card(pool, req.card).await? else {
        return Err(ScanError::UnknownCard);
    };

    // Jadwal pesantren dicatat dalam waktu lokal (WIB).
    let now = Utc::now().with_timezone(&wib());
    let today = now.date_naive();
    let now_time = now.time();

    let schedule = repo::active_schedule_now(pool, user_id, today, now_time).await?;
    let (schedule_id, status, note) = match &schedule {
        Some(s) => {
            let st = if now_time <= s.limit_entry { "present" } else { "late" };
            (Some(s.id), st, None)
        }
        None => (None, "outside_schedule", Some("scan di luar jadwal")),
    };

    // Dedup: satu catatan per jadwal (atau per hari untuk scan bebas).
    if repo::attendance_exists_today(pool, user_id, schedule_id, today).await? {
        return Ok(RfidScanResponse {
            ok: true,
            message: "sudah tercatat sebelumnya".into(),
            student: Some(name),
            status: Some(status.into()),
        });
    }

    // Tautkan ke sesi kelas hari ini bila guru sudah memulai sesi.
    let session_id = match schedule_id {
        Some(sid) => repo::session_for_schedule_today(pool, sid, today).await.unwrap_or(None),
        None => None,
    };

    repo::insert_attendance(pool, user_id, session_id, schedule_id, device.id, &gate, status, note)
        .await?;

    tracing::info!(user_id, card = req.card, gate = %gate, status, "RFID scan tercatat");
    Ok(RfidScanResponse {
        ok: true,
        message: "tercatat".into(),
        student: Some(name),
        status: Some(status.into()),
    })
}

/// Data halaman verifikasi pamong (antrean + jumlah disetujui hari ini, paralel).
pub async fn pamong_data(pool: &Pool) -> Result<PamongData> {
    let (pending, approved_today) =
        tokio::join!(repo::pending_pamong(pool, 50), repo::approved_today(pool));

    let pending = pending?
        .into_iter()
        .map(|p| PendingAtt {
            id: p.id,
            name: p.full_name,
            nis: p.nis.unwrap_or_else(|| "-".into()),
            class_name: p.class_name.unwrap_or_else(|| "-".into()),
            time_label: fmt_when(p.scanned_at),
            gate: p.gate_label.unwrap_or_else(|| "-".into()),
        })
        .collect();

    Ok(PamongData {
        pending,
        approved_today: approved_today?,
    })
}

/// Setujui/tolak satu absensi (tahap pamong).
pub async fn decide_pamong(pool: &Pool, att_id: i64, approver: i64, approve: bool) -> Result<bool> {
    repo::decide_pamong(pool, att_id, approver, approve).await
}

// ── Verifikasi TAHAP 2 (dewan guru) ──────────────────────────────────────────────

/// Antrean verifikasi final + jumlah terverifikasi hari ini. Reuse PamongData
/// (pending + count) — `approved_today` di sini bermakna "terverifikasi hari ini".
pub async fn verify_data(pool: &Pool) -> Result<PamongData> {
    let (pending, verified_today) =
        tokio::join!(repo::pending_verify(pool, 50), repo::verified_today(pool));
    let pending = pending?
        .into_iter()
        .map(|p| PendingAtt {
            id: p.id,
            name: p.full_name,
            nis: p.nis.unwrap_or_else(|| "-".into()),
            class_name: p.class_name.unwrap_or_else(|| "-".into()),
            time_label: fmt_when(p.scanned_at),
            gate: p.gate_label.unwrap_or_else(|| "-".into()),
        })
        .collect();
    Ok(PamongData {
        pending,
        approved_today: verified_today?,
    })
}

/// Verifikasi final satu absensi (tahap 2).
pub async fn decide_verify(pool: &Pool, att_id: i64, approver: i64, approve: bool) -> Result<bool> {
    repo::decide_verify(pool, att_id, approver, approve).await
}
