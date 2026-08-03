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
use axum::extract::DefaultBodyLimit;
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

    // Isi hash kunci perangkat RFID yang masih plaintext (migrasi 53). Sekali
    // jalan; setelah semua terisi, kolom plaintext boleh di-drop. Kegagalan di
    // sini TIDAK boleh menggagalkan start — perangkat lama masih bisa dilayani
    // selama kolom plaintext masih ada.
    match ppm::repository::backfill_api_key_hashes(&pool).await {
        Ok(n) if n > 0 => tracing::info!("Backfill kunci perangkat: {n} di-hash"),
        Ok(_) => {}
        Err(e) => tracing::warn!("Backfill kunci perangkat gagal: {e}"),
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

    // Redis: link undangan registrasi + pending registration/OTP (WAJIB,
    // fail-fast — lihat config.rs). Retry beberapa kali krn Redis kadang
    // start belakangan dari app saat deploy bareng docker-compose.
    let redis_client = redis::Client::open(cfg.redis_url.as_str())?;
    let redis = redis::aio::ConnectionManager::new_with_config(
        redis_client,
        redis::aio::ConnectionManagerConfig::new()
            .set_response_timeout(Some(std::time::Duration::from_secs(10)))
            .set_connection_timeout(Some(std::time::Duration::from_secs(10)))
            .set_number_of_retries(3),
    )
    .await?;
    tracing::info!("Redis ready");

    // HTTP client dipakai ulang (pool koneksi) utk panggil WAHA.
    let http = reqwest::Client::builder()
        .pool_idle_timeout(Some(std::time::Duration::from_secs(30)))
        .timeout(std::time::Duration::from_secs(15))
        .build()?;
    let waha = Arc::new(cfg.waha);

    // JwtService: key di-pre-compute sekali (pola e-ticketing).
    let state = Arc::new(AppState::new(
        pool,
        ppm::auth::JwtService::new(&cfg.jwt_secret),
        storage,
        redis,
        http,
        waha,
    ));

    // ── Telegram alert (error API + monitor WAHA) ────────────────────────────
    // Aktif bila TELEGRAM_BOT_TOKEN & TELEGRAM_ADMIN_CHAT_ID di-set. Setelah
    // di-init, service::telegram::report_error(...) dari mana pun akan kirim WA
    // alert (dedup 60 dtk). Plus monitor WAHA (cek tiap 60 dtk).
    if !cfg.telegram.bot_token.is_empty() && cfg.telegram.admin_chat_id != 0 {
        ppm::service::telegram::init_telegram(ppm::service::telegram::TelegramService::new(
            cfg.telegram.bot_token.clone(),
            cfg.telegram.admin_chat_id,
        ));
        tracing::info!(chat_id = cfg.telegram.admin_chat_id, "Telegram alert aktif");

        let http = state.http.clone();
        let waha = state.waha.clone();
        tokio::spawn(async move {
            use ppm::service::{registration, telegram};
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
            let mut healthy = true; // asumsikan sehat di awal → alert hanya saat transisi
            let mut down_ticks: u32 = 0;
            loop {
                tick.tick().await;
                match registration::waha_status(&http, &waha).await {
                    Ok(_) => {
                        if !healthy {
                            healthy = true;
                            down_ticks = 0;
                            if let Some(tg) = telegram::global() {
                                tg.send_info("✅ <b>WAHA PULIH</b> — WhatsApp kembali WORKING.").await;
                            }
                        }
                    }
                    Err(reason) => {
                        // Alert saat baru down, lalu ~tiap 30 menit selama masih down.
                        if healthy || down_ticks % 30 == 0 {
                            telegram::report_error(0, "WAHA down", reason);
                        }
                        healthy = false;
                        down_ticks = down_ticks.saturating_add(1);
                    }
                }
            }
        });
    }

    // ── Job AUTO-ABSENT / "Alpa" (task internal) ─────────────────────────────
    // Tiap 24 JAM: tandai 'absent' utk santri tanpa kejelasan (bukan hadir/
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
                    Err(e) => {
                        tracing::warn!("Auto-absent gagal: {e}");
                        ppm::service::telegram::report_background_error("Auto-absent", e.to_string());
                    }
                }
                // Materialisasi sesi mendatang SEMUA kelas di sini (di luar jalur
                // request) — dulu dilakukan tiap GET /kelas/:id → lambat.
                match ppm::service::kelas::ensure_upcoming_all(&pool).await {
                    Ok(n) if n > 0 => tracing::info!("Materialisasi sesi: {n} sesi baru"),
                    Ok(_) => {}
                    Err(e) => tracing::warn!("Materialisasi sesi gagal: {e}"),
                }
                // Auto-verify: antrean pamong/dewan guru yang >1 hari tak disentuh
                // manusia disetujui otomatis oleh sistem (lihat
                // repository::run_auto_verify_pamong/_final) — poin tetap diberikan
                // sesuai aturan (termasuk override late_points per jadwal), sama
                // seperti approve manual, agar antrean tak menumpuk selamanya.
                match ppm::repository::run_auto_verify_pamong(&pool).await {
                    Ok(n) if n > 0 => tracing::info!("Auto-verify pamong: {n} absensi disetujui"),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("Auto-verify pamong gagal: {e}");
                        ppm::service::telegram::report_background_error("Auto-verify pamong", e.to_string());
                    }
                }
                match ppm::repository::run_auto_verify_final(&pool).await {
                    Ok(n) if n > 0 => tracing::info!("Auto-verify dewan guru: {n} absensi diverifikasi final"),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("Auto-verify dewan guru gagal: {e}");
                        ppm::service::telegram::report_background_error("Auto-verify final", e.to_string());
                    }
                }
            }
        });
    }

    // ── Job pengingat sesi ke PAMONG (WA, ~1 jam sebelum sesi) ───────────────
    // Tiap 5 menit: kirim WA ke pamong kelas untuk sesi yang mulai ≤60 menit
    // lagi (agar mengatur dewan guru pengisi). Idempotent (repo menandai
    // pamong_notified_at saat klaim → tak kirim ganda). Migrasi 30.
    {
        let pool = state.pool.clone();
        let http = state.http.clone();
        let waha = state.waha.clone();
        let base_url = std::env::var("APP_BASE_URL").unwrap_or_default();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(std::time::Duration::from_secs(300));
            loop {
                tick.tick().await;
                match ppm::service::sessions::send_pamong_session_reminders(
                    &pool, &http, &waha, &base_url,
                )
                .await
                {
                    Ok(n) if n > 0 => tracing::info!("Pengingat sesi pamong: {n} WA terkirim"),
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("Pengingat sesi pamong gagal: {e}");
                        ppm::service::telegram::report_background_error("Pengingat sesi pamong", e.to_string());
                    }
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
    // /pkg: hash-files DIMATIKAN (Cargo.toml) → nama file TETAP (ppm.js/wasm/css).
    // Nama sama walau isi berubah → JANGAN immutable (nanti stale setelah deploy).
    // Pakai `no-cache, must-revalidate`: browser tetap simpan tapi revalidate →
    // 304 kalau tak berubah (hemat byte), ambil baru setelah build baru.
    // /fonts: dari `public/` (assets-dir), nama tetap → sama, no-cache/revalidate.
    let site_root = leptos_options.site_root.to_string();
    let static_routes: axum::Router = axum::Router::new()
        .nest_service(
            "/pkg",
            tower_http::services::ServeDir::new(format!("{site_root}/pkg")),
        )
        .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache, must-revalidate"),
        ))
        .merge(
            axum::Router::new()
                .nest_service(
                    "/fonts",
                    tower_http::services::ServeDir::new(format!("{site_root}/fonts")),
                )
                .layer(tower_http::set_header::SetResponseHeaderLayer::overriding(
                    axum::http::header::CACHE_CONTROL,
                    axum::http::HeaderValue::from_static("no-cache, must-revalidate"),
                )),
        );

    let leptos_router: axum::Router = axum::Router::new()
        .leptos_routes(&leptos_options, ssr_routes, {
            let opts = leptos_options.clone();
            move || shell(opts.clone())
        })
        .fallback(leptos_axum::file_and_error_handler(shell))
        // AppState untuk server functions (diekstrak via Extension).
        .layer(axum::Extension(state.clone()))
        .with_state(leptos_options)
        // Dokumen HTML shell TAK boleh di-cache diam-diam: kalau Safari menyimpan
        // HTML lama, ia menunjuk hash /pkg lama → aset 404 → blank putih setelah
        // redeploy. `no-cache` = boleh disimpan tapi WAJIB revalidasi (304 murah).
        // Aset /pkg ber-hash tetap `immutable` (router terpisah, tak terpengaruh).
        // if_not_present → tak menimpa header yang mungkin diset leptos.
        .layer(tower_http::set_header::SetResponseHeaderLayer::if_not_present(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-cache, must-revalidate"),
        ));

    // ── Endpoint perangkat RFID (gerbang) ────────────────────────────────────
    let device_routes: axum::Router = axum::Router::new()
        .route("/api/rfid/scan", post(ppm::device_api::rfid_scan))
        .route("/api/rfid/gate", post(ppm::device_api::rfid_gate))
        .layer(axum::Extension(state.clone()));

    // ── Batas ukuran body per rute ───────────────────────────────────────────
    // WAJIB DISETEL EKSPLISIT: tanpa layer ini berlaku bawaan axum, yaitu 2 MB —
    // jauh di bawah batas yang divalidasi (dan dijanjikan ke pengguna) di dalam
    // handler. Angkanya ada di web::limits, satu tempat bersama handler-nya,
    // supaya keduanya tak bisa diam-diam berbeda lagi.
    use ppm::web::limits;

    // ── Siaran suara sesi (chunked HTTP; file = rekaman) ─────────────────────
    use ppm::web::live_audio;
    let live_audio_routes: axum::Router = axum::Router::new()
        .route(
            "/api/live-audio/{id}/chunk",
            post(live_audio::post_chunk)
                .layer(DefaultBodyLimit::max(limits::body_limit(limits::AUDIO_CHUNK_MAX))),
        )
        .route("/api/live-audio/{id}/data", get(live_audio::get_data))
        .route("/api/live-audio/{id}/download", get(live_audio::download))
        .route("/api/live-events/{id}", get(ppm::web::live_events::events))
        .layer(axum::Extension(state.clone()));

    // ── Upload file Materials Library (multipart, di luar server-fn) ─────────
    let materials_routes: axum::Router = axum::Router::new()
        .route(
            "/api/materials/upload",
            post(ppm::web::materials::upload)
                .layer(DefaultBodyLimit::max(limits::body_limit(limits::MATERIAL_MAX))),
        )
        .route(
            "/api/activity-photos/upload",
            post(ppm::web::activity_photos::upload)
                .layer(DefaultBodyLimit::max(limits::body_limit(limits::IMAGE_MAX))),
        )
        .route(
            "/api/guestbook",
            post(ppm::web::guestbook::checkin)
                .layer(DefaultBodyLimit::max(limits::body_limit(limits::IMAGE_MAX))),
        )
        .route(
            "/api/bills/proof",
            post(ppm::web::bills::upload_proof)
                .layer(DefaultBodyLimit::max(limits::body_limit(limits::IMAGE_MAX))),
        )
        .layer(axum::Extension(state.clone()));

    // ── Unduh laporan PDF/Excel (biner, di luar server-fn) ───────────────────
    let export_routes: axum::Router = axum::Router::new()
        .route("/api/export/laporan", get(ppm::web::export::download))
        .layer(axum::Extension(state.clone()));

    let app: axum::Router = axum::Router::new()
        .route("/healthz", get(|| async { "ok" }))
        .merge(device_routes)
        .merge(live_audio_routes)
        .merge(materials_routes)
        .merge(export_routes)
        .merge(static_routes)
        .merge(leptos_router)
        .layer(tower_http::compression::CompressionLayer::new())
        // Log akses. Fitur `trace` tower-http sudah ikut ter-compile sejak awal
        // tapi tak pernah dipasang — biayanya dibayar tanpa manfaat. Level DEBUG
        // untuk request (tak membanjiri log produksi; EnvFilter default `ppm=info`
        // menyembunyikannya) dan respons diringkas pada level INFO hanya bila
        // lambat/gagal, lewat filter bawaan tower-http.
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(
                    tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::DEBUG),
                )
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::DEBUG),
                ),
        );

    let listener = TcpListener::bind(socket_addr).await?;
    tracing::info!("ppm (SSR) listening on http://{}", bind_addr);

    axum::serve(listener, app).await?;
    Ok(())
}
