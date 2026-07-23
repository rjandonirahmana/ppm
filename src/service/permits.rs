//! service/permits.rs — Antrean izin sisi PAMONG/dewan guru/admin (migrasi 17,
//! tahap 2). Tahap 1 (konfirmasi orang tua) ada di service::parent.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use super::fmt::{fmt_range, fmt_when};
use crate::models::{permit_kind_label, PermitQueueData, PermitReviewItem};
use crate::repository as repo;

/// Payload halaman /izin-staf: antrean izin yang SUDAH lolos tahap orang tua,
/// menunggu keputusan pamong/dewan guru/admin.
pub async fn permit_queue(pool: &Pool) -> Result<PermitQueueData> {
    let (pending, decided_today) = tokio::join!(
        repo::pending_pamong_permits(pool, 50),
        repo::pamong_permits_decided_today(pool),
    );
    let pending = pending?;
    let pending_count = pending.len() as i64;
    let items = pending
        .into_iter()
        .map(|p| PermitReviewItem {
            id: p.id,
            student_name: p.student_name,
            nis: p.nis.unwrap_or_else(|| "-".into()),
            class_name: p.class_name.unwrap_or_else(|| "-".into()),
            kind_label: permit_kind_label(&p.kind).into(),
            range_label: fmt_range(p.start_date, p.end_date),
            reason: p.reason,
            when_label: fmt_when(p.created_at),
        })
        .collect();

    Ok(PermitQueueData {
        pending_count,
        approved_today: decided_today?,
        items,
    })
}

/// Setujui/tolak izin (pamong/dewan guru/admin).
pub async fn decide_permit(pool: &Pool, permit_id: i64, approve: bool, staff_id: i64) -> Result<()> {
    if !repo::decide_pamong_permit(pool, permit_id, approve, staff_id).await? {
        bail!("Izin tidak ditemukan atau sudah diproses.");
    }
    Ok(())
}
