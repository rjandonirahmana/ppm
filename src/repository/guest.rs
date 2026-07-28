//! repository/guest.rs — Buku tamu (migrasi 35). Baris dibuat saat mesin IoT
//! berhasil check-in tamu (kode cocok di Redis + wajah terunggah).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub async fn insert_guest_visit(
    pool: &Pool,
    name: &str,
    phone: &str,
    purpose: &str,
    face_url: Option<&str>,
    device_id: Option<i64>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO guest_visits (name, phone, purpose, face_url, device_id) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[&name, &phone, &purpose, &face_url, &device_id],
        )
        .await
        .context("insert_guest_visit")?;
    Ok(row.get(0))
}
