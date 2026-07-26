//! service/permits.rs — Antrean izin sisi staf. Alur global bisa dikonfigurasi
//! admin (setelan `permit_approval_mode`):
//!   * two_stage   → Orang Tua → PAMONG (tahap 1) → GURU (final).
//!   * direct_guru → Orang Tua → GURU (final, pamong dilewati).
//! Tahap konfirmasi orang tua ada di service::parent.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use super::fmt::{fmt_range, fmt_when};
use crate::models::{permit_kind_label, PermitQueueData, PermitReviewItem};
use crate::repository as repo;

/// Kunci setelan mode persetujuan izin.
pub const PERMIT_MODE_KEY: &str = "permit_approval_mode";

/// Mode persetujuan izin saat ini ("two_stage" | "direct_guru"). Default
/// "two_stage" bila belum diset.
pub async fn approval_mode(pool: &Pool) -> String {
    repo::get_setting(pool, PERMIT_MODE_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "two_stage".to_string())
}

/// true bila mode = two_stage (pamong dulu, baru guru).
pub async fn is_two_stage(pool: &Pool) -> bool {
    approval_mode(pool).await == "two_stage"
}

/// Ubah mode persetujuan izin (admin). Validasi nilai.
pub async fn set_approval_mode(pool: &Pool, mode: &str) -> Result<()> {
    if !matches!(mode, "two_stage" | "direct_guru") {
        bail!("Mode persetujuan tidak valid.");
    }
    repo::set_setting(pool, PERMIT_MODE_KEY, mode).await
}

fn to_review_items(rows: Vec<repo::PendingPamongRow>) -> Vec<PermitReviewItem> {
    rows.into_iter()
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
        .collect()
}

/// Payload /izin-staf disesuaikan PERAN peninjau (rute PER-KELAS, migrasi 29):
/// - supervisor (pamong) → izin kelas yang WAJIB via pamong (require_pamong);
/// - teacher (wali kelas) → izin santri KELAS-nya (wali_kelas_id = dia);
/// - dewan_guru/admin → SEMUA izin tahap final (oversight).
/// `default_require` (mode global /setelan) = fallback santri tanpa kelas utama.
pub async fn permit_queue(pool: &Pool, role: &str, user_id: i64) -> Result<PermitQueueData> {
    let default_require = is_two_stage(pool).await;

    if role == "supervisor" {
        // Pamong hanya lihat izin santri KELAS yang ia ampu (pamong_id = dia).
        let pamong_id = Some(user_id);
        let (pending, decided_today) = tokio::join!(
            repo::pending_pamong_permits(pool, default_require, pamong_id, 50),
            repo::pamong_permits_decided_today(pool, pamong_id),
        );
        let items = to_review_items(pending?);
        return Ok(PermitQueueData {
            pending_count: items.len() as i64,
            approved_today: decided_today?,
            items,
            two_stage: default_require,
            stage_label: "Persetujuan Pamong".into(),
        });
    }

    // Wali kelas (teacher) → hanya kelasnya; dewan guru/admin → semua.
    let wali_id = (role == "teacher").then_some(user_id);
    let (pending, decided_today) = tokio::join!(
        repo::pending_guru_permits(pool, wali_id, default_require, 50),
        repo::guru_permits_decided_today(pool, wali_id),
    );
    let items = to_review_items(pending?);
    Ok(PermitQueueData {
        pending_count: items.len() as i64,
        approved_today: decided_today?,
        items,
        two_stage: default_require,
        stage_label: "Persetujuan Wali Kelas".into(),
    })
}

/// Setujui/tolak izin. Rute sesuai peran: pamong → tahap 1 (hanya kelas
/// require_pamong); teacher → keputusan final HANYA izin kelasnya; dewan
/// guru/admin → keputusan final izin mana pun.
pub async fn decide_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    role: &str,
    staff_id: i64,
) -> Result<()> {
    let default_require = is_two_stage(pool).await;
    let ok = if role == "supervisor" {
        repo::decide_pamong_permit(pool, permit_id, approve, default_require, Some(staff_id), staff_id)
            .await?
    } else {
        let wali_id = (role == "teacher").then_some(staff_id);
        repo::decide_guru_permit(pool, permit_id, approve, wali_id, default_require, staff_id).await?
    };
    if !ok {
        bail!("Izin tidak ditemukan, sudah diproses, atau di luar wewenang Anda.");
    }
    Ok(())
}
