//! repository/permits.rs — Query tabel permit_requests (izin/sakit).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use deadpool_postgres::Pool;

pub async fn insert_permit(
    pool: &Pool,
    user_id: i64,
    kind: &str,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    reason: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO permit_requests (user_id, type, reason, start_date, end_date) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[&user_id, &kind, &reason, &start_date, &end_date],
        )
        .await
        .context("insert_permit")?;
    Ok(row.get(0))
}

pub struct PermitRow {
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub status: String,
}

pub async fn list_my_permits(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<PermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT type, start_date, end_date, status FROM permit_requests \
             WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2",
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
            status: r.get(3),
        })
        .collect())
}
