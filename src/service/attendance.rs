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
pub async fn record_scan(
    pool: &Pool,
    req: &RfidScanRequest,
) -> Result<RfidScanResponse, ScanError> {
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
            let st = if now_time <= s.limit_entry {
                "present"
            } else {
                "late"
            };
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
        Some(sid) => repo::session_for_schedule_today(pool, sid, today)
            .await
            .unwrap_or(None),
        None => None,
    };

    repo::insert_attendance(
        pool,
        user_id,
        session_id,
        schedule_id,
        device.id,
        &gate,
        status,
        note,
    )
    .await?;

    tracing::info!(user_id, card = req.card, gate = %gate, status, "RFID scan tercatat");
    Ok(RfidScanResponse {
        ok: true,
        message: "tercatat".into(),
        student: Some(name),
        status: Some(status.into()),
    })
}

/// Data halaman verifikasi pamong: antrean + statistik hari ini + sesi hari ini
/// + kehadiran terbaru (dashboard pamong ala mockup).
pub async fn pamong_data(pool: &Pool, pamong_id: Option<i64>) -> Result<PamongData> {
    let (pending, approved_today, stats, today, latest) = tokio::join!(
        repo::pending_pamong(pool, pamong_id, 50),
        repo::approved_today(pool),
        repo::staf_stats(pool),
        repo::today_sessions(pool, 5),
        repo::latest_attendance(pool, 6),
    );

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

    let (total_santri, _growth, hadir_today, _izin) = stats?;
    let pct = if total_santri > 0 {
        ((hadir_today * 100) / total_santri) as i32
    } else {
        0
    };

    Ok(PamongData {
        pending,
        approved_today: approved_today?,
        total_santri,
        hadir_today,
        pct,
        today: super::dashboard::map_live(today?),
        latest: super::dashboard::map_latest(latest?),
    })
}

/// Setujui/tolak satu absensi (tahap pamong). `pamong_id` Some = guard hanya
/// kelas yang diampu guru ini (migrasi 30).
pub async fn decide_pamong(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    pamong_id: Option<i64>,
) -> Result<bool> {
    repo::decide_pamong(pool, att_id, approver, approve, pamong_id).await
}

// ── Verifikasi TAHAP 2 (dewan guru) ──────────────────────────────────────────────

/// Antrean verifikasi final + jumlah terverifikasi hari ini. Reuse PamongData
/// (pending + count) — `approved_today` di sini bermakna "terverifikasi hari ini".
pub async fn verify_data(pool: &Pool, teacher_id: Option<i64>) -> Result<PamongData> {
    let (pending, verified_today, stats) = tokio::join!(
        repo::pending_verify(pool, teacher_id, 50),
        repo::verified_today(pool),
        repo::staf_stats(pool),
    );
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
    let (total_santri, _g, hadir_today, _i) = stats?;
    let pct = if total_santri > 0 {
        ((hadir_today * 100) / total_santri) as i32
    } else {
        0
    };
    Ok(PamongData {
        pending,
        approved_today: verified_today?,
        total_santri,
        hadir_today,
        pct,
        today: vec![],
        latest: vec![],
    })
}

/// Verifikasi final satu absensi (tahap final, oleh ustad bertugas). `teacher_id`
/// Some = guard hanya sesi yang ustadnya guru ini (migrasi 33).
pub async fn decide_verify(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    teacher_id: Option<i64>,
) -> Result<bool> {
    repo::decide_verify(pool, att_id, approver, approve, teacher_id).await
}
