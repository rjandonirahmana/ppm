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
        println!(
            "{}",
            if bcrypt::verify(pw, hash)? {
                "COCOK"
            } else {
                "TIDAK COCOK"
            }
        );
        return Ok(());
    }

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| "ppm=info,tower_http=info".into()),
        )
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

    // RustFS untuk rekaman siaran (opsional). aws-sdk-s3 pakai rustls →
    // crypto provider (ring) wajib di-install sekali sebelum klien dibuat.
    let storage = match &cfg.rustfs {
        Some(rc) => {
            let _ = rustls::crypto::ring::default_provider().install_default();
            let s = Arc::new(ppm::service::storage::StorageService::new(rc));
            if let Err(e) = s.init().await {
                tracing::warn!("RustFS init gagal (lanjut jalan, rekaman bisa gagal pindah): {e}");
            }
            Some(s)
        }
        None => {
            tracing::info!("RustFS nonaktif (RUSTFS_ENDPOINT tak diset) — rekaman disimpan lokal");
            None
        }
    };

    // JwtService: key di-pre-compute sekali (pola e-ticketing).
    let state = Arc::new(AppState::new(
        pool,
        ppm::auth::JwtService::new(&cfg.jwt_secret),
        storage,
    ));

    // ── Job AUTO-ABSENT / "Alpa" (task internal) ─────────────────────────────
    // Tiap 10 menit: tandai 'absent' utk santri tanpa kejelasan (bukan hadir/
    // izin) pada sesi yang sudah TUNTAS (termasuk sesi ad-hoc tanpa jadwal —
    // lihat repository::run_auto_absent). Set-based & idempotent (aman
    // dijalankan berulang). Bisa diganti cron eksternal via endpoint di
    // kemudian hari.
    {
        let pool = state.pool.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(86400));
            loop {
                tick.tick().await;
                match ppm::repository::run_auto_absent(&pool).await {
                    Ok(n) if n > 0 => {
                        tracing::info!("Auto-absent: {n} santri ditandai tidak hadir")
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Auto-absent gagal: {e}"),
                }
                // Materialisasi sesi mendatang SEMUA kelas di sini (di luar jalur
                // request) — dulu dilakukan tiap GET /kelas/:id → lambat.
                match ppm::service::kelas::ensure_upcoming_all(&pool).await {
                    Ok(n) if n > 0 => tracing::info!("Materialisasi sesi: {n} sesi baru"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Materialisasi sesi gagal: {e}"),
                }
            }
        });
    }

    let leptos_conf = get_configuration(Some("Cargo.toml"))
        .map_err(|e| anyhow::anyhow!("gagal load config leptos: {e}"))?;
    let leptos_options = leptos_conf.leptos_options;
    let bind_addr = format!("{}:{}", cfg.host, cfg.port);
    let socket_addr: std::net::SocketAddr = bind_addr
        .parse()
        .map_err(|e| anyhow::anyhow!("alamat bind {bind_addr} tidak valid: {e}"))?;

    let ssr_routes = generate_route_list(App);

    // ── Aset statis (/pkg wasm+js+css, /fonts) dgn Cache-Control ─────────────
    // Filename TIDAK di-hash (cargo-leptos hash-files off) — jadi TIDAK aman
    // di-cache dgn max-age (browser bisa pakai wasm/js LAMA dari cache sampai
    // masa berlaku habis, padahal server sudah rebuild dgn bentuk komponen
    // BEDA → hydration mismatch, gejalanya App "gagal" tak jelas mis. sesi
    // seperti tak tersimpan. PERNAH KEJADIAN di dev (max-age=3600 sempat
    // dipasang, menyebabkan browser nyangkut di WASM basi saat `cargo leptos
    // watch` rebuild berkali-kali) — makanya `no-cache` (BUKAN no-store):
    // browser tetap boleh simpan salinan tapi WAJIB revalidate ke server tiap
    // kali (conditional GET → 304 kalau belum berubah, hemat transfer BYTE
    // tanpa risiko stale). Baru aman pakai max-age/immutable kalau nanti
    // cargo-leptos hash-files diaktifkan (filename berubah tiap build).
    let site_root = leptos_options.site_root.to_string();
    let static_routes: axum::Router = axum::Router::new()
        .nest_service(
            "/pkg",
            tower_http::services::ServeDir::new(format!("{site_root}/pkg")),
        )
        .nest_service(
            "/fonts",
            tower_http::services::ServeDir::new(format!("{site_root}/fonts")),
        )
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache, must-revalidate"),
        ));

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
        .route("/api/rfid/gate", post(ppm::device_api::rfid_gate))
        .layer(axum::Extension(state.clone()));

    // ── Siaran suara sesi (chunked HTTP; file = rekaman) ─────────────────────
    use ppm::web::live_audio;
    let live_audio_routes: axum::Router = axum::Router::new()
        .route("/api/live-audio/{id}/chunk", post(live_audio::post_chunk))
        .route("/api/live-audio/{id}/data", get(live_audio::get_data))
        .route("/api/live-audio/{id}/download", get(live_audio::download))
        .route("/api/live-events/{id}", get(ppm::web::live_events::events))
        .layer(axum::Extension(state.clone()));

    let app: axum::Router = axum::Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(device_routes)
        .merge(live_audio_routes)
        .merge(static_routes)
        .merge(leptos_router)
        .layer(tower_http::compression::CompressionLayer::new());

    let listener = TcpListener::bind(socket_addr).await?;
    tracing::info!("ppm (SSR) listening on http://{}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
