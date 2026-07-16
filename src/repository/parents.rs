//! repository/parents.rs — Query koneksi orang tua ↔ santri (parent_connections).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

/// Cari santri berdasar nama (ILIKE) atau NIS persis. Untuk form koneksi ortu.
pub struct StudentRow {
    pub id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
}

pub async fn search_students(pool: &Pool, q: &str, limit: i64) -> Result<Vec<StudentRow>> {
    let c = pool.get().await?;
    let pattern = format!("%{}%", q.trim());
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, c.name \
             FROM users u \
             LEFT JOIN class_participants cp ON cp.user_id = u.id AND cp.is_primary \
             LEFT JOIN classes c ON c.id = cp.class_id \
             WHERE u.role = 'santri' AND u.is_active = TRUE \
               AND (u.full_name ILIKE $1 OR u.nis = $2) \
             ORDER BY u.full_name LIMIT $3",
            &[&pattern, &q.trim(), &limit],
        )
        .await
        .context("search_students")?;
    Ok(rows
        .into_iter()
        .map(|r| StudentRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
        })
        .collect())
}

pub struct ConnRow {
    pub id: i64,
    pub student_id: i64,
    pub student_name: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
}

/// Semua koneksi milik satu orang tua (connected + pending).
pub async fn connections_of_parent(pool: &Pool, parent_id: i64) -> Result<Vec<ConnRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT pc.id, pc.student_id, u.full_name, pc.status, pc.requested_at \
             FROM parent_connections pc \
             JOIN users u ON u.id = pc.student_id \
             WHERE pc.parent_id = $1 AND pc.status IN ('pending','connected') \
             ORDER BY pc.requested_at ASC",
            &[&parent_id],
        )
        .await
        .context("connections_of_parent")?;
    Ok(rows
        .into_iter()
        .map(|r| ConnRow {
            id: r.get(0),
            student_id: r.get(1),
            student_name: r.get(2),
            status: r.get(3),
            requested_at: r.get(4),
        })
        .collect())
}

/// Kirim permintaan koneksi. Return false bila sudah ada (pending/connected).
pub async fn insert_connection(pool: &Pool, parent_id: i64, student_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO parent_connections (parent_id, student_id) VALUES ($1, $2) \
             ON CONFLICT (parent_id, student_id) DO NOTHING",
            &[&parent_id, &student_id],
        )
        .await
        .context("insert_connection")?;
    Ok(n > 0)
}

/// Apakah ortu terhubung (connected) ke santri ini? Guard akses data anak.
pub async fn is_connected(pool: &Pool, parent_id: i64, student_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT 1 FROM parent_connections \
             WHERE parent_id = $1 AND student_id = $2 AND status = 'connected'",
            &[&parent_id, &student_id],
        )
        .await?;
    Ok(row.is_some())
}

pub struct IncomingReq {
    pub id: i64,
    pub parent_name: String,
    pub requested_at: DateTime<Utc>,
}

/// Permintaan koneksi MASUK untuk seorang santri (menunggu persetujuannya).
pub async fn pending_for_student(pool: &Pool, student_id: i64) -> Result<Vec<IncomingReq>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT pc.id, u.full_name, pc.requested_at \
             FROM parent_connections pc \
             JOIN users u ON u.id = pc.parent_id \
             WHERE pc.student_id = $1 AND pc.status = 'pending' \
             ORDER BY pc.requested_at ASC",
            &[&student_id],
        )
        .await
        .context("pending_for_student")?;
    Ok(rows
        .into_iter()
        .map(|r| IncomingReq {
            id: r.get(0),
            parent_name: r.get(1),
            requested_at: r.get(2),
        })
        .collect())
}

/// Santri menyetujui/menolak permintaan. Return true bila ada yang ter-update.
pub async fn respond_connection(
    pool: &Pool,
    conn_id: i64,
    student_id: i64,
    approve: bool,
) -> Result<bool> {
    let c = pool.get().await?;
    let status = if approve { "connected" } else { "rejected" };
    let n = c
        .execute(
            "UPDATE parent_connections SET status = $3, responded_at = NOW() \
             WHERE id = $1 AND student_id = $2 AND status = 'pending'",
            &[&conn_id, &student_id, &status],
        )
        .await
        .context("respond_connection")?;
    Ok(n > 0)
}

/// Info dasar anak (nama, nis, kelas utama).
pub async fn child_info(pool: &Pool, student_id: i64) -> Result<Option<StudentRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT u.id, u.full_name, u.nis, c.name \
             FROM users u \
             LEFT JOIN class_participants cp ON cp.user_id = u.id AND cp.is_primary \
             LEFT JOIN classes c ON c.id = cp.class_id \
             WHERE u.id = $1 AND u.role = 'santri'",
            &[&student_id],
        )
        .await?;
    Ok(row.map(|r| StudentRow {
        id: r.get(0),
        full_name: r.get(1),
        nis: r.get(2),
        class_name: r.get(3),
    }))
}

pub struct ParentPermitRow {
    pub child_name: String,
    pub kind: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: Option<chrono::NaiveDate>,
    pub reason: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

/// Semua izin milik anak-anak yang terhubung ke ortu ini (terbaru dulu).
pub async fn permits_of_children(pool: &Pool, parent_id: i64, limit: i64) -> Result<Vec<ParentPermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, p.type, p.start_date, p.end_date, p.reason, p.status, p.created_at \
             FROM permit_requests p \
             JOIN parent_connections pc ON pc.student_id = p.user_id \
                  AND pc.parent_id = $1 AND pc.status = 'connected' \
             JOIN users u ON u.id = p.user_id \
             ORDER BY p.created_at DESC LIMIT $2",
            &[&parent_id, &limit],
        )
        .await
        .context("permits_of_children")?;
    Ok(rows
        .into_iter()
        .map(|r| ParentPermitRow {
            child_name: r.get(0),
            kind: r.get(1),
            start_date: r.get(2),
            end_date: r.get(3),
            reason: r.get(4),
            status: r.get(5),
            created_at: r.get(6),
        })
        .collect())
}
