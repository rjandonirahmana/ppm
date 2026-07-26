//! repository/settings.rs — Setelan global aplikasi (tabel key-value app_settings).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

/// Ambil nilai setelan; None bila kunci belum ada.
pub async fn get_setting(pool: &Pool, key: &str) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT value FROM app_settings WHERE key = $1", &[&key])
        .await
        .context("get_setting")?;
    Ok(row.map(|r| r.get(0)))
}

/// Simpan/perbarui setelan (upsert).
pub async fn set_setting(pool: &Pool, key: &str, value: &str) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO app_settings (key, value, updated_at) VALUES ($1, $2, NOW()) \
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = NOW()",
        &[&key, &value],
    )
    .await
    .context("set_setting")?;
    Ok(())
}
