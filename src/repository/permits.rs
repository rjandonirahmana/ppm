//! repository/permits.rs — Query tabel permit_requests (izin/sakit/keperluan).
//!
//! Migrasi 17: dua tahap — Orang Tua (parent_status) → Pamong (pamong_status).
//! Permohonan diajukan SANTRI sendiri butuh konfirmasi orang tua dulu
//! (parent_status='pending'); diajukan LANGSUNG oleh orang tua otomatis lolos
//! tahap ini (parent_status='approved' saat insert).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use deadpool_postgres::Pool;

#[allow(clippy::too_many_arguments)]
pub async fn insert_permit(
    pool: &Pool,
    user_id: i64,
    requested_by: i64,
    kind: &str,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    reason: &str,
    parent_status: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO permit_requests \
                (user_id, requested_by, type, reason, start_date, end_date, parent_status) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
            &[&user_id, &requested_by, &kind, &reason, &start_date, &end_date, &parent_status],
        )
        .await
        .context("insert_permit")?;
    Ok(row.get(0))
}

pub struct PermitRow {
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub parent_status: String,
    pub pamong_status: String,
}

pub async fn list_my_permits(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<PermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT type, start_date, end_date, parent_status, pamong_status \
             FROM permit_requests WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
            &[&user_id, &limit],
        )
        .await
        .context("list_my_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PermitRow {
            kind: r.get(0),
            start_date: r.get(1),
            end_date: r.get(2),
            parent_status: r.get(3),
            pamong_status: r.get(4),
        })
        .collect())
}

// ── Tahap 1: konfirmasi ORANG TUA ────────────────────────────────────────────

pub struct PendingParentRow {
    pub id: i64,
    pub child_name: String,
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub reason: String,
}

/// Permit anak (yang TERHUBUNG ke `parent_id`) diajukan SANTRI SENDIRI
/// (requested_by = user_id) dan masih menunggu konfirmasi orang tua.
pub async fn pending_parent_confirms(pool: &Pool, parent_id: i64) -> Result<Vec<PendingParentRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT p.id, u.full_name, p.type, p.start_date, p.end_date, p.reason \
             FROM permit_requests p \
             JOIN users u ON u.id = p.user_id \
             JOIN parent_connections pc ON pc.student_id = p.user_id AND pc.parent_id = $1 \
             WHERE p.requested_by = p.user_id AND p.parent_status = 'pending' \
                AND pc.status = 'connected' \
             ORDER BY p.created_at DESC",
            &[&parent_id],
        )
        .await
        .context("pending_parent_confirms")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingParentRow {
            id: r.get(0),
            child_name: r.get(1),
            kind: r.get(2),
            start_date: r.get(3),
            end_date: r.get(4),
            reason: r.get(5),
        })
        .collect())
}

/// Konfirmasi/tolak izin oleh orang tua. Guard kepemilikan LANGSUNG di query
/// (parent_connections terhubung ke santri pemilik izin) — bukan cuma di
/// layer service — supaya orang tua tak bisa menebak `permit_id` milik anak
/// orang lain. Return true bila baris ter-update.
pub async fn confirm_parent_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    parent_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p SET parent_status = $2, parent_confirmed_by = $3, \
                parent_confirmed_at = NOW() \
             WHERE p.id = $1 AND p.parent_status = 'pending' \
                AND p.requested_by = p.user_id \
                AND EXISTS ( \
                    SELECT 1 FROM parent_connections pc \
                    WHERE pc.student_id = p.user_id AND pc.parent_id = $3 AND pc.status = 'connected' \
                )",
            &[&permit_id, &status, &parent_id],
        )
        .await
        .context("confirm_parent_permit")?;
    Ok(n > 0)
}

// ── Tahap 2: PAMONG/dewan guru ────────────────────────────────────────────────

pub struct PendingPamongRow {
    pub id: i64,
    pub student_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Antrean pamong: sudah lolos tahap orang tua, menunggu keputusan pamong.
pub async fn pending_pamong_permits(pool: &Pool, limit: i64) -> Result<Vec<PendingPamongRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT p.id, u.full_name, u.nis, \
                (SELECT c.name FROM class_participants cp JOIN classes c ON c.id = cp.class_id \
                    WHERE cp.user_id = u.id LIMIT 1), \
                p.type, p.start_date, p.end_date, p.reason, p.created_at \
             FROM permit_requests p JOIN users u ON u.id = p.user_id \
             WHERE p.parent_status = 'approved' AND p.pamong_status = 'pending' \
             ORDER BY p.created_at ASC LIMIT $1",
            &[&limit],
        )
        .await
        .context("pending_pamong_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingPamongRow {
            id: r.get(0),
            student_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kind: r.get(4),
            start_date: r.get(5),
            end_date: r.get(6),
            reason: r.get(7),
            created_at: r.get(8),
        })
        .collect())
}

/// Jumlah izin diputuskan pamong HARI INI (statistik antrean).
pub async fn pamong_permits_decided_today(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FROM permit_requests \
             WHERE pamong_status <> 'pending' \
                AND (pamong_at AT TIME ZONE 'Asia/Jakarta')::date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date",
            &[],
        )
        .await
        .context("pamong_permits_decided_today")?;
    Ok(row.get(0))
}

/// Setujui/tolak izin oleh pamong/dewan guru/admin.
pub async fn decide_pamong_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    staff_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests SET pamong_status = $2, pamong_by = $3, pamong_at = NOW() \
             WHERE id = $1 AND parent_status = 'approved' AND pamong_status = 'pending'",
            &[&permit_id, &status, &staff_id],
        )
        .await
        .context("decide_pamong_permit")?;
    Ok(n > 0)
}
