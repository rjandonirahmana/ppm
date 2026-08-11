//! service/recording.rs — Finalisasi rekaman: file lokal → RustFS.
//!
//! Dipicu saat staf MENGAKHIRI sesi (set_session_live start=false). Selama
//! siaran, file lokal (web/live_audio.rs) adalah sumber kebenaran untuk poll
//! santri; setelah selesai barulah dipindah ke RustFS dengan key
//! `ppm/<kelas>/<sesi>/<tanggal>-sesi-<id>.webm`, kolom recording_* diarahkan
//! ke URL publik, lalu file lokal dihapus. RustFS nonaktif → tetap lokal
//! (recording_path sudah menunjuk /api/live-audio/{id}/download).

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;

use crate::state::AppState;

/// Sidik tak-tertebak untuk key rekaman sebuah sesi (16 hex = 64 bit).
///
/// MASALAH yang diselesaikan: key rekaman seluruhnya bisa ditebak —
/// `recordings/<kelas>/<judul>/<tanggal>-sesi-<id>.webm`, dengan `id` yang
/// berurutan. Bucket RustFS-nya publik (lihat `.env.example`), jadi endpoint
/// `/api/live-audio/{id}/download` yang dijaga ketat itu bisa dilewati begitu
/// saja: siapa pun tanpa login cukup menebak nama kelas dan menaikkan nomor
/// sesi untuk mengunduh rekaman kajian — suara orang sungguhan.
///
/// Diturunkan dari `JWT_SECRET` lewat HMAC, BUKAN diacak lalu disimpan:
///   • tak perlu kolom baru dan tak perlu migrasi;
///   • hasilnya TETAP untuk sesi yang sama, jadi finalisasi ulang menimpa objek
///     yang sama persis — sifat idempoten yang memang diandalkan `upload_file`.
/// Konsekuensinya sama dengan token langganan .ics: mengganti `JWT_SECRET`
/// membuat seluruh URL rekaman lama tak lagi cocok dengan yang akan dihasilkan
/// (berkas lamanya masih ada, hanya tak ditulis ulang) — itu tombol pencabutan
/// massalnya.
///
/// 64 bit sudah jauh di luar jangkauan tebakan untuk lalu lintas HTTP nyata,
/// dan menjaga nama berkasnya tetap terbaca manusia.
fn sidik_rekaman(jwt_secret: &str, session_id: i64) -> String {
    let tag = super::ics::hmac_sha256(
        jwt_secret.as_bytes(),
        format!("rekaman:v1:{session_id}").as_bytes(),
    );
    tag[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// Slug segmen key S3: huruf kecil, alfanumerik dipertahankan, sisanya '-',
/// rangkap dilipat. Non-ASCII dibuang (key S3 aman & URL bersih).
fn slug(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut dash = true; // tahan '-' di awal
    for c in s.chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            dash = false;
        } else if !dash {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "tanpa-nama".into()
    } else {
        out
    }
}

/// Fire-and-forget: jalankan finalisasi di background, error cukup di-log
/// (mengakhiri sesi tidak boleh gagal hanya karena storage).
pub fn finalize_async(state: Arc<AppState>, session_id: i64) {
    tokio::spawn(async move {
        if let Err(e) = finalize(&state, session_id).await {
            tracing::warn!(session_id, "finalisasi rekaman gagal: {e:#}");
        }
    });
}

async fn finalize(state: &Arc<AppState>, session_id: i64) -> Result<()> {
    let Some(storage) = state.storage.clone() else {
        return Ok(()); // RustFS nonaktif → rekaman tetap lokal
    };
    let path = crate::web::live_audio::recording_file(session_id);
    if tokio::fs::metadata(&path).await.is_err() {
        return Ok(()); // sesi tanpa siaran suara
    }

    // Klien guru mungkin masih menguras antrean chunk (retry setelah putus).
    // Tunggu ukuran file STABIL 6 dtk berturut-turut; menyerah setelah ~90 dtk.
    let mut last = 0u64;
    let mut stable = 0u32;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let size = tokio::fs::metadata(&path).await.map(|m| m.len()).unwrap_or(0);
        if size > 0 && size == last {
            stable += 1;
            if stable >= 2 {
                break;
            }
        } else {
            stable = 0;
            last = size;
        }
    }

    let d = crate::repository::session_detail(&state.pool, session_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("sesi tidak ditemukan"))?;
    // Prefix `ppm/` DIBUANG: bucket sudah bernama "ppm" → dulu jadi `/ppm/ppm/...`.
    //
    // Sidik di ujung nama membuat key ini tak bisa ditebak dari luar (lihat
    // `sidik_rekaman`) tanpa mengorbankan keterbacaannya bagi pengurus yang
    // membuka bucket, dan tanpa mengorbankan sifat idempotennya.
    let key = format!(
        "recordings/{}/{}/{}-sesi-{}-{}.webm",
        slug(&d.class_name),
        slug(d.title.as_deref().unwrap_or("sesi")),
        d.session_date, // YYYY-MM-DD
        session_id,
        sidik_rekaman(&state.jwt_secret, session_id),
    );
    let size = tokio::fs::metadata(&path).await?.len() as i64;
    let url = storage.upload_file(&path, &key, "audio/webm").await?;
    crate::repository::update_recording(&state.pool, session_id, &url, "audio/webm", size).await?;
    // Siarannya ditutup SESUDAH alamatnya menunjuk RustFS dan SEBELUM berkas
    // lokalnya dihapus. Urutan ini yang membuat potongan susulan tak bisa lagi
    // menghidupkan kembali berkas lokal — lihat `live_audio::siaran_selesai`.
    crate::web::live_audio::siaran_selesai(session_id);
    let _ = tokio::fs::remove_file(&path).await;
    state.notify_live(session_id); // SSE → kartu "Unduh Rekaman" muncul live
    tracing::info!(session_id, %url, size, "rekaman dipindah ke RustFS");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn slug_bersih() {
        assert_eq!(slug("Tahfidz Al-Qur'an  (Pagi)"), "tahfidz-al-qur-an-pagi");
        assert_eq!(slug("---"), "tanpa-nama");
        assert_eq!(slug("Kajian Subuh"), "kajian-subuh");
    }
}
