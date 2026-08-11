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

/// Peran yang memang mengawasi SELURUH kelas — tak perlu keterkaitan data.
fn is_pengawas(role: &str) -> bool {
    matches!(role, "admin" | "ketua" | "dewan_guru")
}

/// Gerbang tunggal semua pintu sesi: siaran, dengar, unduh.
///
/// Dulu `get_data` dan `download` hanya menuntut token yang sah. Artinya
/// santri mana pun cukup mengganti angka di URL untuk mendengarkan — atau
/// mengunduh — rekaman kelas lain; dan rekaman pengajian itu isinya orang
/// betulan, bukan berkas anonim.
///
/// Kegagalan DB → TOLAK (503), bukan fail-open. Beda dari cek kategori di
/// `post_chunk` yang sengaja fail-open: di sana yang dipertaruhkan hanya siaran
/// yang bisa diulang, di sini isi rekaman orang lain.
async fn boleh_akses_sesi(state: &AppState, claims: &Claims, session_id: i64) -> Option<StatusCode> {
    // Kelas non-KBM tak punya rekaman sama sekali (migrasi 65) — pintunya
    // ditutup untuk SEMUA orang, termasuk pengawas. Kalau ada berkas tertinggal
    // dari sebelum aturan ini, ia tak lagi bisa diambil siapa pun.
    match crate::repository::sesi_kelas_kbm(&state.pool, session_id).await {
        Ok(true) => {}
        Ok(false) => {
            tracing::warn!(session_id, "tolak akses rekaman: kelas ini bukan KBM");
            return Some(StatusCode::FORBIDDEN);
        }
        Err(e) => {
            tracing::error!(session_id, "cek kategori sesi gagal (tolak): {e}");
            return Some(StatusCode::SERVICE_UNAVAILABLE);
        }
    }
    if is_pengawas(&claims.role) {
        return None;
    }
    match crate::repository::session_stakeholder(&state.pool, session_id, claims.user_id).await {
        Ok(true) => None,
        Ok(false) => {
            tracing::warn!(
                session_id,
                user_id = claims.user_id,
                "tolak akses sesi: bukan pihak yang berkepentingan"
            );
            Some(StatusCode::FORBIDDEN)
        }
        Err(e) => {
            tracing::error!(session_id, "cek akses sesi gagal (tolak): {e}");
            Some(StatusCode::SERVICE_UNAVAILABLE)
        }
    }
}

#[derive(Deserialize)]
pub struct ChunkQ {
    pub seq: u64,
}

/// Siaran yang sedang berjalan, dicatat di memori proses saat `seq = 0`.
///
/// Kenapa di memori dan bukan DB: potongan datang tiap ~4 detik per penyiar,
/// dan modul ini sengaja dirancang agar potongan SUSULAN tak bergantung pada
/// layanan lain (lihat catatan di `post_chunk`). Yang disimpan hanya dua fakta
/// yang tak bisa disimpulkan dari berkas: SIAPA yang memulai, dan potongan
/// ke berapa yang ditunggu berikutnya.
struct Siaran {
    pemilik: i64,
    /// Nomor potongan yang ditunggu. Bukan sekadar penghitung: inilah yang
    /// membedakan kiriman ulang (jaringan putus setelah server menulis tapi
    /// sebelum jawabannya sampai) dari potongan baru. Tanpa ini, satu retry
    /// menyisipkan potongan yang sama dua kali dan rekamannya cacat.
    seq_berikut: u64,
    sentuh: std::time::Instant,
}

/// Umur siaran tak tersentuh sebelum catatannya dibuang. Longgar: siaran yang
/// masih hidup menyentuhnya tiap ~4 detik, dan membuang catatan siaran yang
/// masih berjalan memaksa penyiarnya mengulang dari awal.
const SIARAN_IDLE: std::time::Duration = std::time::Duration::from_secs(600);

fn siaran_map() -> &'static std::sync::Mutex<std::collections::HashMap<i64, Siaran>> {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};
    static M: OnceLock<Mutex<HashMap<i64, Siaran>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Catat awal siaran baru (seq = 0) — menimpa catatan lama sesi itu.
fn siaran_mulai(session_id: i64, pemilik: i64) {
    let now = std::time::Instant::now();
    if let Ok(mut m) = siaran_map().lock() {
        m.retain(|_, s| now.duration_since(s.sentuh) < SIARAN_IDLE);
        m.insert(session_id, Siaran { pemilik, seq_berikut: 1, sentuh: now });
    }
}

/// Tutup siaran sebuah sesi — dipanggil `service::recording` SESUDAH rekamannya
/// selesai diunggah ke penyimpanan objek.
///
/// KENAPA PENTING. Finalisasi menunggu berkasnya berhenti tumbuh (sampai ~90
/// detik) supaya potongan susulan dari klien yang jaringannya putus tetap
/// tertampung — itu disengaja. Tapi begitu unggahan selesai, berkas lokalnya
/// DIHAPUS dan `recording_path` diarahkan ke URL RustFS. Potongan yang datang
/// SESUDAH itu dulu masih diterima: berkas lokalnya lahir kembali berisi satu
/// serpihan yang tak bisa diputar, dan `post_chunk` menimpa `recording_path`
/// kembali ke `/download` — rekaman yang sudah terunggah jadi tak terjangkau
/// siapa pun, dan yang tersaji di layar adalah serpihan tadi.
///
/// Dengan catatannya dibuang di sini, potongan susulan jatuh ke cabang "tak ada
/// catatan" di [`siaran_lanjut`] dan dijawab 409 — klien berhenti mengirim,
/// tak ada berkas yang lahir kembali.
pub(crate) fn siaran_selesai(session_id: i64) {
    if let Ok(mut m) = siaran_map().lock() {
        m.remove(&session_id);
    }
}

/// Kunci tulis PER SESI untuk berkas rekaman.
///
/// `siaran_mulai`/`siaran_lanjut` hanya menjaga catatan di memori, lalu
/// melepaskan kuncinya SEBELUM berkasnya disentuh. Dua request yang datang
/// bersamaan karena itu bisa menulis ke berkas yang sama pada saat yang sama:
///   • dua `seq = 0` (klik ganda, atau retry klien yang memang dirancang ada)
///     saling memotong berkas dan sama-sama menulis di offset 0 — rekamannya
///     rusak sejak byte pertama, tepat di header WebM yang menentukan bisa
///     tidaknya berkas itu diputar sama sekali;
///   • dua `append` bisa saling menyisip.
///
/// `tokio::sync::Mutex`, bukan `std::sync::Mutex`: kuncinya harus tetap
/// dipegang melintasi `await` saat menulis ke disk.
fn kunci_tulis(session_id: i64) -> std::sync::Arc<tokio::sync::Mutex<()>> {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock};
    static M: OnceLock<Mutex<HashMap<i64, Arc<tokio::sync::Mutex<()>>>>> = OnceLock::new();
    let m = M.get_or_init(|| Mutex::new(HashMap::new()));
    let Ok(mut m) = m.lock() else {
        // Kunci teracuni → jangan matikan siaran; beri kunci sekali pakai.
        return Arc::new(tokio::sync::Mutex::new(()));
    };
    // Sesi yang siarannya sudah tak tercatat lagi tak perlu kuncinya disimpan.
    // Dibersihkan di sini, bukan lewat penyapu berkala: satu-satunya yang
    // menambah isi peta ini adalah fungsi ini sendiri.
    if m.len() > 64 {
        if let Ok(siaran) = siaran_map().lock() {
            m.retain(|id, _| siaran.contains_key(id));
        }
    }
    m.entry(session_id).or_insert_with(|| Arc::new(tokio::sync::Mutex::new(()))).clone()
}

/// Keputusan untuk potongan lanjutan (seq > 0).
enum Lanjutan {
    /// Tulis potongan ini, lalu naikkan penghitung.
    Tulis,
    /// Sudah pernah ditulis (kiriman ulang) — jawab OK tanpa menulis apa pun.
    Duplikat,
    Tolak(StatusCode),
}

fn siaran_lanjut(session_id: i64, pengirim: i64, seq: u64) -> Lanjutan {
    let Ok(mut m) = siaran_map().lock() else {
        // Kunci teracuni → jangan matikan siaran yang sedang jalan.
        return Lanjutan::Tulis;
    };
    let Some(s) = m.get_mut(&session_id) else {
        // Tak ada catatan: proses baru saja restart, atau potongan menyusul
        // siaran yang sudah lama berhenti. Menuliskannya ke ekor berkas hanya
        // menghasilkan rekaman yang tak bisa diputar — suruh klien mulai dari
        // seq 0 supaya header WebM-nya utuh.
        return Lanjutan::Tolak(StatusCode::CONFLICT);
    };
    if s.pemilik != pengirim {
        // Kepemilikan dulu HANYA diperiksa di seq 0, jadi staf lain bisa
        // menempelkan suaranya ke rekaman yang sedang berjalan.
        return Lanjutan::Tolak(StatusCode::FORBIDDEN);
    }
    s.sentuh = std::time::Instant::now();
    match seq.cmp(&s.seq_berikut) {
        std::cmp::Ordering::Equal => {
            s.seq_berikut += 1;
            Lanjutan::Tulis
        }
        std::cmp::Ordering::Less => Lanjutan::Duplikat,
        // Ada potongan yang hilang di tengah. Menerimanya berarti menambal
        // lubang dengan audio yang salah posisi; lebih baik klien mengirim
        // ulang yang tertinggal.
        std::cmp::Ordering::Greater => Lanjutan::Tolak(StatusCode::CONFLICT),
    }
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
    // untuk kelas non-KBM, tapi endpoint tetap tolak di server — jangan percaya
    // klien bisa dipaksa kirim request langsung. Cek HANYA di seq=0 (awal
    // siaran; kategori kelas tak berubah di tengah siaran) — bukan tiap
    // potongan, agar potongan susulan TIDAK bergantung DB (filosofi modul ini:
    // siaran jalan lewat file lokal, tahan gangguan).
    //
    // Kegagalan DB di sini MENOLAK, tak lagi fail-open seperti dulu: hanya KBM
    // yang boleh punya berkas rekaman sama sekali (migrasi 65), dan berkas yang
    // terlanjur lahir di kelas piket tak bisa "dibatalkan" belakangan —
    // sementara siaran yang gagal mulai tinggal dicoba lagi.
    if q.seq == 0 {
        match crate::repository::sesi_kelas_kbm(&state.pool, session_id).await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(session_id, "tolak siaran: kelas ini bukan KBM");
                return StatusCode::FORBIDDEN.into_response();
            }
            Err(e) => {
                tracing::error!(session_id, "cek kategori sesi gagal (tolak): {e}");
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
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
        match crate::repository::session_broadcasters(&state.pool, session_id).await {
            Ok(Some((teacher, pamong, wali, status))) => {
                // Sesi yang sudah selesai/batal tak menerima siaran lagi —
                // rekamannya sudah dipindah dan berkas lokalnya dihapus, jadi
                // potongan susulan hanya melahirkan berkas yatim yang isinya
                // bertentangan dengan yang tercatat di DB.
                if matches!(status.as_str(), "finished" | "cancelled") {
                    tracing::warn!(session_id, %status, "tolak siaran: sesi sudah berakhir");
                    return StatusCode::GONE.into_response();
                }
                if !is_pengawas(&claims.role) {
                    let me = Some(claims.user_id);
                    if teacher != me && pamong != me && wali != me {
                        tracing::warn!(
                            session_id, user_id = claims.user_id,
                            "tolak siaran: bukan pengisi/pamong/wali sesi ini"
                        );
                        return StatusCode::FORBIDDEN.into_response();
                    }
                }
            }
            Ok(None) => return StatusCode::NOT_FOUND.into_response(),
            Err(e) => {
                tracing::error!(session_id, "cek kepemilikan sesi gagal (tolak): {e}");
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
        }
        siaran_mulai(session_id, claims.user_id);
    } else {
        // Potongan lanjutan: kepemilikan & urutan diperiksa dari catatan di
        // memori — tanpa menyentuh DB, sesuai filosofi modul ini.
        match siaran_lanjut(session_id, claims.user_id, q.seq) {
            Lanjutan::Tulis => {}
            Lanjutan::Duplikat => {
                // Kiriman ulang yang sah. Dijawab OK supaya klien maju ke
                // potongan berikutnya alih-alih mengulang selamanya.
                return (StatusCode::OK, [("x-duplicate", "1")]).into_response();
            }
            Lanjutan::Tolak(s) => {
                tracing::warn!(
                    session_id, user_id = claims.user_id, seq = q.seq,
                    "tolak potongan lanjutan: {s}"
                );
                return s.into_response();
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
    //
    // Seluruh operasi berkas berada DI DALAM kunci per-sesi: memotong lalu
    // menulis adalah dua langkah, dan dua request bersamaan yang menyelinginya
    // menghasilkan rekaman rusak sejak byte pertama (lihat `kunci_tulis`).
    let kunci = kunci_tulis(session_id);
    let _jaga = kunci.lock().await;
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
            //
            // `update_recording_lokal`, bukan `update_recording`: yang ini
            // MENOLAK menimpa alamat yang sudah menunjuk penyimpanan objek.
            // Tanpa itu, satu potongan susulan yang lolos setelah finalisasi
            // mengembalikan alamat rekaman ke berkas lokal yang isinya tinggal
            // serpihan — dan rekaman yang sudah terunggah hilang dari jangkauan.
            let size = meta.len() as i64;
            let web_path = format!("/api/live-audio/{session_id}/download");
            let _ = crate::repository::update_recording_lokal(
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
    let claims = match auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if let Some(s) = boleh_akses_sesi(&state, &claims, session_id).await {
        return s.into_response();
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

/// GET /api/live-audio/{id}/download — unduh rekaman penuh.
///
/// Hanya pihak yang berkepentingan atas sesi ini (lihat `boleh_akses_sesi`).
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
    let claims = match auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if let Some(s) = boleh_akses_sesi(&state, &claims, session_id).await {
        return s.into_response();
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
