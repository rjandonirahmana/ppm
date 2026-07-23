//! repository/materials.rs — Query tabel materials (Materials Library, migrasi
//! 17). Diunggah manual staf; class_id opsional (bisa lintas-kelas/umum).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub struct MaterialRow {
    pub id: i64,
    pub title: String,
    pub kind: String,
    pub file_url: String,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn recent_materials(pool: &Pool, limit: i64) -> Result<Vec<MaterialRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, title, kind, file_url, mime_type, file_size, created_at \
             FROM materials ORDER BY created_at DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("recent_materials")?;
    Ok(rows
        .into_iter()
        .map(|r| MaterialRow {
            id: r.get(0),
            title: r.get(1),
            kind: r.get(2),
            file_url: r.get(3),
            mime_type: r.get(4),
            file_size: r.get(5),
            created_at: r.get(6),
        })
        .collect())
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_material(
    pool: &Pool,
    class_id: Option<i64>,
    uploaded_by: i64,
    title: &str,
    kind: &str,
    file_url: &str,
    mime_type: Option<&str>,
    file_size: Option<i64>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO materials (class_id, uploaded_by, title, kind, file_url, mime_type, file_size) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id",
            &[&class_id, &uploaded_by, &title, &kind, &file_url, &mime_type, &file_size],
        )
        .await
        .context("insert_material")?;
    Ok(row.get(0))
}

pub async fn delete_material(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c.execute("DELETE FROM materials WHERE id = $1", &[&id]).await.context("delete_material")?;
    Ok(n > 0)
}
