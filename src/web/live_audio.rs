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

use std::io::SeekFrom;
use std::path::PathBuf;
use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

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

/// Jendela & jatah potongan siaran per (pengguna, sesi).
///
/// Sengaja LONGGAR. AudioDock mengirim ±1 potongan per 4 detik (≈15/menit),
/// tapi lihat catatan modul di atas: saat internet putus potongan MENGANTRE di
/// klien lalu dikirim beruntun begitu sambungan pulih. Batas yang pas-pasan
/// akan menolak justru kiriman susulan yang SAH, dan lubang di rekaman tak bisa
/// diperbaiki belakangan — sementara kiriman yang membanjir paling banter
/// memboroskan I/O disk. Asimetri itu yang menentukan angkanya: 60/menit
/// memberi ruang mengejar ketertinggalan ±3 menit, sekaligus tetap memberi
/// atap alih-alih membiarkannya tanpa batas sama sekali.
const CHUNK_WINDOW: std::time::Duration = std::time::Duration::from_secs(60);
const CHUNK_QUOTA: u32 = 60;

/// Apakah potongan ini masih di dalam jatah? Jendela tetap (fixed window) per
/// (pengguna, sesi), disimpan di memori proses.
///
/// Tidak memakai Redis meski tersedia: modul ini sengaja dirancang agar
/// potongan susulan TIDAK bergantung pada layanan lain (lihat filosofi di
/// `post_chunk`), jadi pembatas lajunya pun tak boleh jadi titik gagal baru.
/// Konsekuensinya batas ini per-proses — memadai karena ppm berjalan sebagai
/// satu proses.
fn chunk_rate_ok(user_id: i64, session_id: i64) -> bool {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    use std::time::Instant;

    static SLOT: OnceLock<Mutex<HashMap<(i64, i64), (Instant, u32)>>> = OnceLock::new();
    let slot = SLOT.get_or_init(|| Mutex::new(HashMap::new()));
    // Mutex teracuni → jangan matikan siaran hanya karena pembatas lajunya
    // rusak; ini lapis tambahan, bukan gerbang utama.
    let Ok(mut m) = slot.lock() else { return true };

    let now = Instant::now();
    // Tanpa pembersihan ini peta tumbuh selamanya seiring sesi baru berdatangan.
    m.retain(|_, (mulai, _)| now.duration_since(*mulai) < CHUNK_WINDOW);

    let e = m.entry((user_id, session_id)).or_insert((now, 0));
    if now.duration_since(e.0) >= CHUNK_WINDOW {
        *e = (now, 0);
    }
    e.1 += 1;
    e.1 <= CHUNK_QUOTA
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
    if body.is_empty() || body.len() > crate::web::limits::AUDIO_CHUNK_MAX {
        return StatusCode::BAD_REQUEST.into_response();
    }
    // Ukuran tiap potongan sudah dibatasi DefaultBodyLimit di router, tapi
    // LAJUNYA belum: tanpa ini satu akun staf bisa mengirim potongan
    // sebanyak-banyaknya dan menghabiskan I/O disk. Dicek setelah auth supaya
    // jatahnya melekat pada pengguna, bukan pada alamat IP yang bisa dibagi
    // banyak orang di jaringan pesantren.
    if !chunk_rate_ok(claims.user_id, session_id) {
        tracing::warn!(session_id, user_id = claims.user_id, "potongan siaran melebihi jatah laju");
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }
    // Pertahanan berlapis: klien (AudioDock) sudah sembunyikan tombol siaran
    // untuk kategori selain "Pengajian", tapi endpoint tetap tolak di server —
    // jangan percaya klien bisa dipaksa kirim request langsung. Cek HANYA di
    // seq=0 (awal siaran; kategori kelas tak berubah di tengah siaran) — bukan
    // tiap potongan, agar potongan susulan TIDAK bergantung DB (filosofi modul
    // ini: siaran jalan lewat file lokal, tahan gangguan). DB tak terjangkau
    // saat cek → fail-OPEN (log saja): ini lapis TAMBAHAN, gerbang utama tetap
    // UI klien + is_staff di atas; hiccup DB tak boleh mematikan seluruh siaran.
    if q.seq == 0 {
        match crate::repository::session_category(&state.pool, session_id).await {
            Ok(cat) if cat.as_deref().is_some_and(|c| !crate::models::category_allows_recording(c)) => {
                return StatusCode::FORBIDDEN.into_response();
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(session_id, "cek kategori sesi gagal (lanjut, fail-open): {e}"),
        }

        // KEPEMILIKAN SESI. Tanpa ini, staf mana pun bisa mengirim seq=0 ke
        // session_id mana pun — dan seq=0 MENIMPA file dari awal, jadi rekaman
        // sesi orang lain hilang tak bisa dikembalikan.
        //
        // Beda dari cek kategori di atas yang fail-OPEN: di sini kegagalan DB
        // membuat kita MENOLAK. Alasannya asimetris — siaran yang gagal mulai
        // tinggal dicoba lagi, sedangkan rekaman yang terlanjur tertimpa tak
        // ada gantinya. Cek hanya di seq=0, jadi potongan susulan tetap tak
        // menyentuh DB (filosofi modul ini dipertahankan).
        if !matches!(claims.role.as_str(), "admin" | "ketua" | "dewan_guru") {
            match crate::repository::session_broadcasters(&state.pool, session_id).await {
                Ok(Some((teacher, pamong, wali))) => {
                    let me = Some(claims.user_id);
                    if teacher != me && pamong != me && wali != me {
                        tracing::warn!(
                            session_id, user_id = claims.user_id,
                            "tolak siaran: bukan pengisi/pamong/wali sesi ini"
                        );
                        return StatusCode::FORBIDDEN.into_response();
                    }
                }
                Ok(None) => return StatusCode::NOT_FOUND.into_response(),
                Err(e) => {
                    tracing::error!(session_id, "cek kepemilikan sesi gagal (tolak): {e}");
                    return StatusCode::SERVICE_UNAVAILABLE.into_response();
                }
            }
        }
    }

    let path = recording_file(session_id);
    if let Some(parent) = path.parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    // seq 0 → truncate (siaran baru); selain itu append.
    //
    // `tokio::fs`, BUKAN `std::fs`: ini handler async yang dipanggil tiap ~4 detik
    // per penyiar, dan tulis ke disk VPS bisa menghentikan thread pemanggilnya
    // selama puluhan milidetik saat disk sibuk. Dengan std::fs, thread yang
    // terhenti itu adalah worker runtime Tokio — di VPS 2 CPU hanya ada segelintir
    // worker, sehingga SELURUH request lain (SSR halaman, server-fn) ikut tertahan.
    let res = async {
        let mut f = tokio::fs::OpenOptions::new()
            .create(true)
            .append(q.seq != 0)
            .write(true)
            .truncate(q.seq == 0)
            .open(&path)
            .await?;
        tokio::io::AsyncWriteExt::write_all(&mut f, &body).await?;
        f.metadata().await
    }
    .await;

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
    // Async sepanjang jalur: endpoint ini di-poll TERUS-MENERUS oleh setiap santri
    // di ruangan selama siaran berlangsung — jalur paling sering dieksekusi di
    // seluruh aplikasi. I/O blocking di sini menahan worker Tokio dikalikan jumlah
    // santri (lihat alasan lengkap di post_chunk).
    let Ok(mut f) = tokio::fs::File::open(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let len = f.metadata().await.map(|m| m.len()).unwrap_or(0);
    let from = q.from.min(len);
    let take = (len - from).min(1_048_576) as usize;
    let mut buf = vec![0u8; take];
    if f.seek(SeekFrom::Start(from)).await.is_err() || f.read_exact(&mut buf).await.is_err() {
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
///
/// DI-STREAM, tidak dibaca sekaligus. Rekaman pengajian 1–2 jam berukuran puluhan
/// MB; `std::fs::read` (versi lama) menaruh SELURUH file di RAM sebelum satu byte
/// pun terkirim, jadi beberapa wali santri yang mengunduh bersamaan bisa
/// menghabiskan memori VPS 4GB — dan pembacaannya blocking, menahan worker Tokio
/// selama itu. `ReaderStream` mengirim per potongan dengan memori tetap kecil.
pub async fn download(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<i64>,
    headers: HeaderMap,
) -> Response {
    if auth(&state, &headers).is_err() {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let path = recording_file(session_id);
    let Ok(file) = tokio::fs::File::open(&path).await else {
        return StatusCode::NOT_FOUND.into_response();
    };
    // Content-Length diisi bila ukuran diketahui → browser bisa menampilkan
    // progres unduhan (tanpa ini hanya "unknown size").
    let len = file.metadata().await.ok().map(|m| m.len());
    let body = Body::from_stream(tokio_util::io::ReaderStream::new(file));

    let mut resp = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "audio/webm")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"sesi-{session_id}.webm\""),
        );
    if let Some(len) = len {
        resp = resp.header(header::CONTENT_LENGTH, len);
    }
    match resp.body(body) {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(session_id, "gagal menyusun respons unduhan: {e}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
