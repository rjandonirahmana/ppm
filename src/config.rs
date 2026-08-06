//! config.rs — Konfigurasi aplikasi + pool Postgres (server-only).

use anyhow::{Context, Result};
use deadpool_postgres::{
    Config as PgConfig, ManagerConfig, Pool, PoolConfig, RecyclingMethod, Runtime,
};
use std::env;
use tokio_postgres::NoTls;

pub struct AppConfig {
    pub host: String,
    pub port: u16,
    pub database_url: String,
    pub db_pool_max_size: usize,
    pub jwt_secret: String,
    /// RustFS (S3-compatible) untuk rekaman siaran — None = simpan lokal saja.
    pub rustfs: Option<RustFsConfig>,
    /// Redis: link undangan registrasi (24 jam) + pending registration/OTP
    /// (10 menit). WAJIB (fail-fast) — tak ada mode "nonaktif" yang masuk akal
    /// utk fitur yang inti kerjanya menyimpan kode OTP sementara.
    pub redis_url: String,
    pub waha: WahaConfig,
    /// Notifikasi error ke Telegram — aktif bila bot_token & admin_chat_id ada.
    pub telegram: TelegramConfig,
}

/// Telegram bot untuk alert error server/WAHA (pola e-ticketing). Nonaktif bila
/// bot_token kosong / admin_chat_id 0.
#[derive(Clone, Default)]
pub struct TelegramConfig {
    pub bot_token: String,
    pub admin_chat_id: i64,
}

/// WAHA (WhatsApp HTTP API) — kirim OTP + password registrasi lewat WA.
/// Pola sama e-ticketing: base_url + session + api_key opsional.
pub struct WahaConfig {
    pub base_url: String,
    pub session: String,
    pub api_key: String,
}

/// Kredensial RustFS (pola e-ticketing/wedding-web). Aktif bila RUSTFS_ENDPOINT
/// di-set. Bucket default "ppm" (bucket sendiri); key tetap berprefix `ppm/...`.
pub struct RustFsConfig {
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
    pub bucket: String,
    pub public_url: String,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".into()),
            port: env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            // WAJIB. Default lama "dev-secret-ubah-di-prod" adalah salah tempel:
            // itu bukan connection string, jadi aplikasi tetap mati saat membuat
            // pool — hanya dengan pesan yang membingungkan. Lebih baik gagal di
            // sini dengan sebab yang jelas.
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL wajib diset (lihat .env.example)")?,
            // Default 16 untuk VPS 4 CPU / 8 GB (naik dari 8 yang disetel saat
            // masih 2 CPU / 4 GB).
            //
            // Patokannya jumlah CPU, bukan jumlah pengguna: query di sini
            // pendek-pendek dan seluruh basis data (±11 MB) muat di cache
            // Postgres, jadi yang membatasi adalah berapa banyak query yang
            // benar-benar bisa jalan bersamaan — sekitar 2–4× core. Pool
            // terlalu besar justru memperburuk: koneksi menganggur memakan
            // slot `max_connections` yang DIPAKAI BERSAMA aplikasi lain di
            // Postgres yang sama.
            //
            // Perhitungan slot: max_connections 100, terpakai ±50 oleh
            // e-ticketing & lainnya. Dua proses ppm (container + dev) × 16 = 32
            // → total ±82, masih di bawah batas. Naikkan lewat DB_POOL_MAX_SIZE
            // hanya bersamaan dengan menaikkan max_connections.
            db_pool_max_size: env::var("DB_POOL_MAX_SIZE")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(16),
            // WAJIB. Dengan default, aplikasi TETAP JALAN dan menerbitkan token
            // yang ditandatangani rahasia yang tertulis di kode sumber — siapa
            // pun bisa memalsukan sesi admin. Gagal-cepat jauh lebih aman
            // daripada berjalan tampak normal.
            jwt_secret: env::var("JWT_SECRET")
                .context("JWT_SECRET wajib diset (lihat .env.example)")?,
            rustfs: match env::var("RUSTFS_ENDPOINT") {
                Ok(endpoint) => Some(RustFsConfig {
                    endpoint,
                    access_key: env::var("RUSTFS_ACCESS_KEY")
                        .context("RUSTFS_ACCESS_KEY wajib bila RUSTFS_ENDPOINT di-set")?,
                    secret_key: env::var("RUSTFS_SECRET_KEY")
                        .context("RUSTFS_SECRET_KEY wajib bila RUSTFS_ENDPOINT di-set")?,
                    bucket: env::var("RUSTFS_BUCKET").unwrap_or_else(|_| "ppm".into()),
                    public_url: env::var("RUSTFS_PUBLIC_URL")
                        .context("RUSTFS_PUBLIC_URL wajib bila RUSTFS_ENDPOINT di-set")?,
                }),
                Err(_) => None,
            },
            redis_url: env::var("REDIS_URL").context("REDIS_URL wajib diset (registrasi+OTP)")?,
            waha: WahaConfig {
                // 3001, BUKAN 3000: port 3000 adalah default aplikasi ini
                // sendiri — default lama membuat WAHA seolah menunjuk ke diri
                // sendiri dan gagal dengan pesan yang membingungkan.
                base_url: env::var("WAHA_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:3001".into()),
                session: env::var("WAHA_SESSION").unwrap_or_else(|_| "default".into()),
                api_key: env::var("WAHA_API_KEY").unwrap_or_default(),
            },
            telegram: TelegramConfig {
                bot_token: env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default(),
                admin_chat_id: env::var("TELEGRAM_ADMIN_CHAT_ID")
                    .ok()
                    .and_then(|s| s.trim().parse().ok())
                    .unwrap_or(0),
            },
        })
    }
}

/// Buat pool Postgres + verifikasi koneksi awal (fail-fast bila DB mati).
pub async fn create_pool(database_url: &str, max_size: usize) -> Result<Pool> {
    let mut cfg = PgConfig::new();
    cfg.url = Some(database_url.to_string());
    cfg.pool = Some(PoolConfig::new(max_size));
    // Verifikasi koneksi SEBELUM dipakai ulang. DB VPS / firewall memutus koneksi
    // yang menganggur → tanpa ini deadpool (default `Fast`) bisa menyerahkan
    // koneksi MATI (client `is_closed()` masih false karena SERVER yang memutus)
    // → query pertama gagal "connection closed/reset". `Verified` melakukan ping
    // ringan (simple_query "") pada recycle sehingga koneksi basi dibuang & dibuat
    // baru → menghilangkan error koneksi intermiten (mis. login setelah app idle).
    cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Verified });
    let pool = cfg
        .create_pool(Some(Runtime::Tokio1), NoTls)
        .context("gagal membuat pool Postgres")?;
    let _ = pool.get().await.context("koneksi awal ke Postgres gagal")?;
    Ok(pool)
}
