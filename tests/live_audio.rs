//! tests/live_audio.rs — Uji endpoint siaran suara TANPA DB.
//!
//! Pool dibuat tapi tidak pernah tersambung (URL port mati) — update_recording
//! memang best-effort, jadi alur chunk/data/download tetap teruji end-to-end:
//! guru kirim potongan berurutan → server APPEND (file = rekaman) → santri poll
//! offset → unduh penuh → siaran ulang (seq=0) men-truncate file.

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::routing::{get, post};
use axum::Extension;
use std::sync::Arc;
use tower::ServiceExt;

use ppm::auth::JwtService;
use ppm::state::AppState;
use ppm::web::live_audio;

fn router(state: Arc<AppState>) -> axum::Router {
    axum::Router::new()
        .route("/api/live-audio/{id}/chunk", post(live_audio::post_chunk))
        .route("/api/live-audio/{id}/data", get(live_audio::get_data))
        .route("/api/live-audio/{id}/download", get(live_audio::download))
        .layer(Extension(state))
}

fn req(method: &str, uri: &str, token: Option<&str>, body: &'static [u8]) -> Request<Body> {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        b = b.header(header::COOKIE, format!("ppm_token={t}"));
    }
    b.body(Body::from(body)).unwrap()
}

async fn body_bytes(resp: axum::response::Response) -> Vec<u8> {
    axum::body::to_bytes(resp.into_body(), 16 * 1024 * 1024).await.unwrap().to_vec()
}

#[tokio::test]
async fn alur_siaran_chunk_data_download() {
    // Direktori rekaman terisolasi per proses uji.
    let dir = std::env::temp_dir().join(format!("ppm-rec-test-{}", std::process::id()));
    std::env::set_var("RECORDINGS_DIR", &dir);

    // Pool valid secara struktur tapi menunjuk port mati → tiap akses DB gagal
    // cepat; handler harus tetap berfungsi (kolom rekaman diisi best-effort).
    let mut cfg = deadpool_postgres::Config::new();
    cfg.url = Some("postgres://x:x@127.0.0.1:1/x".into());
    let pool = cfg
        .create_pool(Some(deadpool_postgres::Runtime::Tokio1), tokio_postgres::NoTls)
        .unwrap();
    let jwt = JwtService::new("secret-uji");
    let guru = jwt.sign(1, "Ustadz Fulan", "0811", "teacher").unwrap();
    let santri = jwt.sign(2, "Santri Fulan", "0822", "santri").unwrap();
    let app = router(Arc::new(AppState::new(pool, jwt, None)));

    // Tanpa login → 401; santri kirim chunk → 403.
    let r = app.clone().oneshot(req("GET", "/api/live-audio/9/data", None, b"")).await.unwrap();
    assert_eq!(r.status(), StatusCode::UNAUTHORIZED);
    let r = app
        .clone()
        .oneshot(req("POST", "/api/live-audio/9/chunk?seq=0", Some(&santri), b"X"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::FORBIDDEN);

    // Belum ada siaran → data 404.
    let r = app
        .clone()
        .oneshot(req("GET", "/api/live-audio/9/data?from=0", Some(&santri), b""))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::NOT_FOUND);

    // Guru: seq=0 lalu seq=1 → server APPEND berurutan.
    for (seq, part) in [(0, b"HEADER".as_slice()), (1, b"WORLD".as_slice())] {
        let r = app
            .clone()
            .oneshot(req(
                "POST",
                &format!("/api/live-audio/9/chunk?seq={seq}"),
                Some(&guru),
                part,
            ))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK, "chunk seq={seq}");
    }

    // Santri poll dari offset 0 → seluruh isi + x-next di ujung file.
    let r = app
        .clone()
        .oneshot(req("GET", "/api/live-audio/9/data?from=0", Some(&santri), b""))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let next: u64 = r.headers()["x-next"].to_str().unwrap().parse().unwrap();
    assert_eq!(next, 11);
    assert_eq!(body_bytes(r).await, b"HEADERWORLD");

    // Poll lanjutan dari offset 6 → hanya sisa.
    let r = app
        .clone()
        .oneshot(req("GET", "/api/live-audio/9/data?from=6", Some(&santri), b""))
        .await
        .unwrap();
    assert_eq!(body_bytes(r).await, b"WORLD");

    // Unduh penuh sebagai lampiran.
    let r = app
        .clone()
        .oneshot(req("GET", "/api/live-audio/9/download", Some(&santri), b""))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert!(r.headers()[header::CONTENT_DISPOSITION].to_str().unwrap().contains("sesi-9.webm"));
    assert_eq!(body_bytes(r).await, b"HEADERWORLD");

    // Siaran BARU: seq=0 men-truncate file lama (header WebM harus di awal).
    let r = app
        .clone()
        .oneshot(req("POST", "/api/live-audio/9/chunk?seq=0", Some(&guru), b"NEW"))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let r = app
        .clone()
        .oneshot(req("GET", "/api/live-audio/9/data?from=0", Some(&santri), b""))
        .await
        .unwrap();
    let next: u64 = r.headers()["x-next"].to_str().unwrap().parse().unwrap();
    assert_eq!(next, 3, "file lama harus terpangkas");
    assert_eq!(body_bytes(r).await, b"NEW");

    let _ = std::fs::remove_dir_all(&dir);
}
