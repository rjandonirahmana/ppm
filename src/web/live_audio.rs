//! web/live_audio.rs — Siaran SUARA sesi (server, axum) — TANPA WebRTC.
//!
//! KEPUTUSAN ARSITEKTUR (vs SFU e-ticketing): kebutuhan ppm = audio SATU ARAH
//! (ustadz → santri), tanya-jawab lewat CHAT teks (sudah ada), WAJIB terekam,
//! dan tahan internet putus. Maka dipakai **chunked audio streaming via HTTP**:
//!   • Guru: MediaRecorder (Opus/WebM) potong ~4 dtk → POST /chunk?seq=N.
//!     Server APPEND ke satu file → FILE ITULAH REKAMANNYA (rekaman gratis,
//!     server = sumber kebenaran). Putus internet → chunk antre di klien,
//!     di-retry berurutan saat tersambung → file tetap kontinu.
//!   • Santri: poll GET /data?from=offset → append ke MediaSource (latensi
//!     ~4–8 dtk — wajar utk ceramah; pertanyaan via chat).
//!   • Selesai sesi → kolom recording_* terisi → tombol unduh di /sesi/:id.
//! Keunggulan vs WebRTC di kasus ini: tanpa UDP/ICE/TURN (aman di WiFi/NAT
//! pesantren), rekaman inheren, jauh lebih sederhana; trade-off: latensi detik
//! (bukan sub-detik) — dapat diterima karena interaksi balik hanya teks.

use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, Query};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use serde::Deserialize;

use crate::models::Claims;
use crate::state::AppState;

/// Direktori penyimpanan rekaman (env RECORDINGS_DIR, default ./recordings).
pub fn recordings_dir() -> PathBuf {
    std::env::var("RECORDINGS_DIR").unwrap_or_else(|_| "recordings".into()).into()
}

pub fn recording_file(session_id: i64) -> PathBuf {
    recordings_dir().join(format!("{session_id}.webm"))
}

/// Auth dari cookie ppm_token (handler axum murni, di luar server-fn).
/// pub(crate): dipakai juga endpoint SSE web/live_events.rs.
pub(crate) fn auth(state: &AppState, headers: &HeaderMap) -> Result<Claims, StatusCode> {
    let cookie = headers.get(header::COOKIE).and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = cookie
        .split(';')
        .filter_map(|p| p.trim().strip_prefix("ppm_token="))
        .next()
        .ok_or(StatusCode::UNAUTHORIZED)?;
    state.jwt.verify(token).map_err(|_| StatusCode::UNAUTHORIZED)
}

fn is_staff(role: &str) -> bool {
    matches!(role, "admin" | "supervisor" | "dewan_guru" | "teacher")
}

#[derive(Deserialize)]
pub struct ChunkQ {
    pub seq: u64,
}

/// POST /api/live-audio/{id}/chunk?seq=N — terima potongan audio dari GURU.
/// seq=0 = mulai siaran BARU → file dibuat ulang (header WebM harus di awal).
pub async fn post_chunk(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<i64>,
    Query(q): Query<ChunkQ>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let claims = match auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if !is_staff(&claims.role) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if body.is_empty() || body.len() > 2_000_000 {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let path = recording_file(session_id);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // seq 0 → truncate (siaran baru); selain itu append.
    let res = std::fs::OpenOptions::new()
        .create(true)
        .append(q.seq != 0)
        .write(true)
        .truncate(q.seq == 0)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, &body).map(|_| f))
        .and_then(|f| f.metadata());

    match res {
        Ok(meta) => {
            // Update kolom rekaman (path web + mime + ukuran) — best effort.
            let size = meta.len() as i64;
            let web_path = format!("/api/live-audio/{session_id}/download");
            let _ = crate::repository::update_recording(
                &state.pool, session_id, &web_path, "audio/webm", size,
            )
            .await;
            (StatusCode::OK, [("x-size", size.to_string())]).into_response()
        }
        Err(e) => {
            tracing::warn!(session_id, "tulis chunk gagal: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
pub struct DataQ {
    #[serde(default)]
    pub from: u64,
}

/// GET /api/live-audio/{id}/data?from=OFFSET — santri poll audio dari offset.
/// Balas bytes [from..from+1MB) + header x-next (offset berikutnya).
pub async fn get_data(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<i64>,
    Query(q): Query<DataQ>,
    headers: HeaderMap,
) -> Response {
    if auth(&state, &headers).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let path = recording_file(session_id);
    let Ok(mut f) = std::fs::File::open(&path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let len = f.metadata().map(|m| m.len()).unwrap_or(0);
    let from = q.from.min(len);
    let take = (len - from).min(1_048_576) as usize;
    let mut buf = vec![0u8; take];
    if f.seek(SeekFrom::Start(from)).is_err() || f.read_exact(&mut buf).is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE.as_str(), "application/octet-stream".to_string()),
            ("x-next", (from + take as u64).to_string()),
            ("x-total", len.to_string()),
        ],
        buf,
    )
        .into_response()
}

/// GET /api/live-audio/{id}/download — unduh rekaman penuh (login apa pun).
pub async fn download(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    if auth(&state, &headers).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    match std::fs::read(recording_file(session_id)) {
        Ok(bytes) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE.as_str(), "audio/webm".to_string()),
                (
                    header::CONTENT_DISPOSITION.as_str(),
                    format!("attachment; filename=\"sesi-{session_id}.webm\""),
                ),
            ],
            bytes,
        )
            .into_response(),
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
