//! main.rs — Server PPM AFM (Leptos SSR + Axum).
//!
//!   cargo leptos watch   # SSR + hydration WASM (dev penuh)
//!   cargo run            # SSR saja (butuh DATABASE_URL di .env)
//!
//! Pola mengikuti proyek Leptos SSR lain di mesin ini (e-ticketing, wedding-web):
//! satu binary Axum, Leptos routes catch-all, AppState via Extension.

#![recursion_limit = "512"]

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use std::sync::Arc;

use anyhow::Result;
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use ppm::config::{create_pool, AppConfig};
use ppm::state::AppState;
use ppm::web::app::{shell, App};

use leptos::config::get_configuration;
use leptos_axum::{generate_route_list, LeptosRoutes};

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    // Utilitas CLI: `cargo run -- hash <password>` → cetak bcrypt hash.
    // Berguna saat mengisi users.password_hash manual lewat SQL.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("hash") {
        let pw = args
            .get(2)
            .expect("pemakaian: cargo run -- hash <password>");
        println!("{}", bcrypt::hash(pw, 10)?);
        return Ok(());
    }
    // `cargo run -- verify <password> <hash>` → cek password cocok dgn hash.
    if args.get(1).map(String::as_str) == Some("verify") {
        let pw = args.get(2).expect("pemakaian: verify <password> <hash>");
        let hash = args.get(3).expect("pemakaian: verify <password> <hash>");
        println!("{}", if bcrypt::verify(pw, hash)? { "COCOK" } else { "TIDAK COCOK" });
        return Ok(());
    }

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "ppm=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cfg = AppConfig::from_env()?;
    tracing::info!(host = %cfg.host, port = cfg.port, "Config loaded");

    let pool = create_pool(&cfg.database_url, cfg.db_pool_max_size).await?;
    tracing::info!("Postgres pool ready (max={})", cfg.db_pool_max_size);

    // Bootstrap: buat admin awal HANYA bila tabel users kosong.
    if let Err(e) = ppm::service::auth::ensure_seed_admin(&pool).await {
        tracing::warn!("Seed admin gagal (lanjut jalan): {e}");
    }

    // JwtService: key di-pre-compute sekali (pola e-ticketing).
    let state = Arc::new(AppState::new(pool, ppm::auth::JwtService::new(&cfg.jwt_secret)));

    let leptos_conf = get_configuration(Some("Cargo.toml"))
        .map_err(|e| anyhow::anyhow!("gagal load config leptos: {e}"))?;
    let leptos_options = leptos_conf.leptos_options;
    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    let socket_addr: std::net::SocketAddr = bind_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("alamat bind {bind_addr} tidak valid: {e}"))?;

    let ssr_routes = generate_route_list(App);

    let leptos_router: axum::Router = axum::Router::new()
        .leptos_routes(&leptos_options, ssr_routes, {
            let opts = leptos_options.clone();
            move || shell(opts.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        // AppState untuk server functions (diekstrak via Extension).
        .layer(axum::Extension(state.clone()))
        .with_state(leptos_options);

    // ── Endpoint perangkat RFID (gerbang) ────────────────────────────────────
    let device_routes: axum::Router = axum::Router::new()
        .route("/api/rfid/scan", post(ppm::device_api::rfid_scan))
        .layer(axum::Extension(state.clone()));

    let app: axum::Router = axum::Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(device_routes)
        .merge(leptos_router)
        .layer(tower_http::compression::CompressionLayer::new());

    let listener = TcpListener::bind(socket_addr).await?;
    tracing::info!("ppm (SSR) listening on http://{}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
