//! config.rs — Konfigurasi aplikasi + pool Postgres (server-only).

use anyhow::{Context, Result};
use deadpool_postgres::{Config as PgConfig, Pool, PoolConfig, Runtime};
use std::env;
use tokio_postgres::NoTls;

pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub db_pool_max_size: usize,
    pub jwt_secret: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            database_url: env::var("DATABASE_URL").context("DATABASE_URL wajib di-set (.env)")?,
            db_pool_max_size: env::var("DB_POOL_MAX_SIZE")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(16),
            jwt_secret: env::var("JWT_SECRET").unwrap_or_else(|_| "dev-secret-ubah-di-prod".into()),
        })
    }
}

/// Buat pool Postgres + verifikasi koneksi awal (fail-fast bila DB mati).
pub async fn create_pool(database_url: &str, max_size: usize) -> Result<Pool> {
    let mut cfg = PgConfig::new();
    cfg.url = Some(database_url.to_string());
    cfg.pool = Some(PoolConfig::new(max_size));
    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .context("gagal membuat pool Postgres")?;
    let _ = pool.get().await.context("koneksi awal ke Postgres gagal")?;
    Ok(pool)
}
