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
use ppm::config::WahaConfig;
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
    //
    // `set_var` aman DI SINI dan hanya di sini: tiap berkas di tests/ jadi
    // binary tersendiri, berkas ini berisi SATU uji, dan variabelnya diset
    // sebelum ada thread lain yang membacanya. Menambah uji kedua ke berkas ini
    // akan mematahkan ketiga syarat itu sekaligus — `cargo test` menjalankan
    // uji satu binary secara paralel, dan `RECORDINGS_DIR` dibaca handler dari
    // env global. Kalau perlu uji kedua, buat berkas tests/ baru.
    let dir = std::env::temp_dir().join(format!("ppm-rec-test-{}", std::process::id()));
    // SAFETY: edisi 2024 menjadikan `set_var` unsafe karena ia mengubah keadaan
    // global proses sementara pustaka C lain bisa membaca `environ` dari thread
    // lain tanpa penguncian. Tiga syarat di atas — satu binary, satu uji, diset
    // sebelum thread lain ada — persis yang membuat balapan itu mustahil di
    // sini. Blok `unsafe` ini menuliskannya, bukan melonggarkannya: kalau uji
    // kedua ditambahkan ke berkas ini, syaratnya batal dan blok ini menjadi
    // salah, bukan sekadar berisik.
    unsafe {
        std::env::set_var("RECORDINGS_DIR", &dir);
    }

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

    // Redis: sama pola dgn pool Postgres di atas — klien valid struktur tapi
    // menunjuk port mati; live_audio TIDAK menyentuh redis sama sekali, jadi
    // ConnectionManager cukup ADA (bukan tersambung sungguhan) utk mengisi
    // AppState. Timeout pendek + nol retry agar setup uji tak menggantung.
    // CATATAN: versi redis ini MENYAMBUNG saat membentuk ConnectionManager
    // (tidak lazy), jadi tanpa Redis lokal uji ini tak bisa berjalan.
    let redis_client = redis::Client::open("redis://127.0.0.1:6379/").unwrap();
    let redis = match redis::aio::ConnectionManager::new_with_config(
        redis_client,
        redis::aio::ConnectionManagerConfig::new()
            .set_connection_timeout(Some(std::time::Duration::from_millis(300)))
            .set_response_timeout(Some(std::time::Duration::from_millis(300)))
            .set_number_of_retries(0),
    )
    .await
    {
        Ok(m) => m,
        // ── KENAPA INI PANIK DI CI ───────────────────────────────────────────
        // Versi lama `return` begitu saja dengan `eprintln!`. Akibatnya uji ini
        // dilaporkan LULUS tanpa menjalankan satu pun assertion di bawahnya —
        // dan di CI (yang memang tak punya redis-server) itu berarti seluruh
        // alur chunk→data→download tak pernah benar-benar diuji, sementara
        // papan hijau mengatakan sebaliknya. Uji yang tak berjalan harus
        // terlihat, bukan menyamar jadi uji yang lulus.
        //
        // Di mesin pengembang melewatinya masih masuk akal (tak semua orang
        // menyalakan Redis untuk menyentuh satu berkas), jadi bedanya cuma di
        // CI. Bila CI kelak menyalakan service Redis, cabang ini tak pernah
        // tersentuh dan boleh dihapus.
        Err(e) => {
            // Gerbangnya `PPM_REQUIRE_REDIS`, BUKAN `CI`. Versi pertama memakai
            // `CI` dan itu terlalu kasar: banyak alat (task runner, beberapa
            // terminal, editor) menyetel `CI=1` untuk urusan lain, sehingga
            // uji ini gagal di mesin pengembang yang memang tak menjalankan
            // Redis — dengan pesan tentang CI yang tak masuk akal di sana.
            //
            // Variabel eksplisit hanya diset oleh workflow yang MEMANG
            // menyediakan service Redis. Jadi maknanya tepat: "di sini Redis
            // dijanjikan ada; kalau tak ada, itu kegagalan sungguhan."
            assert!(
                std::env::var("PPM_REQUIRE_REDIS").is_err(),
                "PPM_REQUIRE_REDIS diset tapi Redis tak bisa dihubungi ({e}). \
                 Di CI berarti service Redis-nya mati/salah port — uji ini WAJIB \
                 berjalan di sana, bukan dilewati diam-diam."
            );
            eprintln!(
                "SKIP alur_siaran_chunk_data_download: Redis lokal tak tersedia ({e}). \
                 Jalankan `redis-server` untuk menjalankan uji ini."
            );
            return;
        }
    };

    let http = reqwest::Client::new();
    let waha = Arc::new(WahaConfig {
        base_url: "http://127.0.0.1:1".into(),
        session: "default".into(),
        api_key: String::new(),
    });
    let app =
        router(Arc::new(AppState::new(pool, jwt, "secret-uji".into(), None, redis, http, waha)));

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
