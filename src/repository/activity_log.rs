//! repository/activity_log.rs — Query tabel activity_logs (migrasi 17, halaman
//! User Control): jejak aksi administratif (ganti peran, aktif/nonaktifkan).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub async fn insert_log(
    pool: &Pool,
    actor_id: i64,
    target_id: Option<i64>,
    action: &str,
    detail: Option<&str>,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO activity_logs (actor_id, target_user_id, action, detail) \
         VALUES ($1, $2, $3, $4)",
        &[&actor_id, &target_id, &action, &detail],
    )
    .await
    .context("insert_log")?;
    Ok(())
}

/// Catat SATU tindakan atas BANYAK sasaran sekaligus — pelaku, aksi, dan
/// keterangannya sama, yang berbeda hanya siapa yang dikenai.
///
/// Bentuknya begini karena itulah kejadian yang sesungguhnya di
/// `service::admin::serahkan_ketua`: satu penyerahan jabatan menurunkan
/// beberapa orang, dan mencatatnya di dalam loop berarti satu INSERT per orang.
pub async fn insert_log_many(
    pool: &Pool,
    actor_id: i64,
    target_ids: &[i64],
    action: &str,
    detail: Option<&str>,
) -> Result<()> {
    if target_ids.is_empty() {
        return Ok(());
    }
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO activity_logs (actor_id, target_user_id, action, detail) \
         SELECT $1, t, $3, $4 FROM unnest($2::bigint[]) AS t",
        &[&actor_id, &target_ids, &action, &detail],
    )
    .await
    .context("insert_log_many")?;
    Ok(())
}

pub struct ActivityLogRow {
    pub actor_name: Option<String>,
    pub target_name: Option<String>,
    pub action: String,
    pub detail: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Log aktivitas dalam `hari` terakhir, terbaru dulu.
///
/// Rentangnya dihitung sargable — `created_at >= NOW() - INTERVAL`, bukan
/// membungkus kolomnya dengan fungsi tanggal. Pola `(created_at AT TIME ZONE
/// …)::date BETWEEN …` yang dipakai di beberapa query lain mematikan index
/// pada kolom itu; di tabel yang tumbuh terus seperti `activity_logs`, itu
/// berarti seluruh tabel dipindai untuk menampilkan tiga hari terakhir.
pub async fn recent_logs(pool: &Pool, hari: i32, limit: i64) -> Result<Vec<ActivityLogRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT a.full_name, t.full_name, l.action, l.detail, l.created_at \
             FROM activity_logs l \
             LEFT JOIN users a ON a.id = l.actor_id \
             LEFT JOIN users t ON t.id = l.target_user_id \
             WHERE l.created_at >= NOW() - make_interval(days => $1) \
             ORDER BY l.created_at DESC LIMIT $2",
            &[&hari, &limit],
        )
        .await
        .context("recent_logs")?;
    Ok(rows
        .into_iter()
        .map(|r| ActivityLogRow {
            actor_name: r.get(0),
            target_name: r.get(1),
            action: r.get(2),
            detail: r.get(3),
            created_at: r.get(4),
        })
        .collect())
}
