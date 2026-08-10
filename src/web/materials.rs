//! web/materials.rs — Upload FILE ke Materials Library (migrasi 17), handler
//! axum murni (di luar server-fn — butuh multipart, sama alasan
//! web/live_audio.rs). Auth cookie manual (admin/dewan_guru saja).
//!
//! `kind='link'` (tautan, mis. YouTube) TIDAK lewat sini — server fn biasa
//! `add_material_link_action` (web/api.rs) sudah cukup, tanpa file.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;

use crate::state::AppState;

fn is_manager(role: &str) -> bool {
    matches!(role, "admin" | "dewan_guru")
}

/// Ekstensi file → (kind, content_type) yang diterima Materials Library.
fn classify(filename: &str) -> Option<(&'static str, &'static str)> {
    let ext = filename.rsplit('.').next()?.to_lowercase();
    Some(match ext.as_str() {
        "mp3" => ("audio", "audio/mpeg"),
        "wav" => ("audio", "audio/wav"),
        "ogg" | "opus" => ("audio", "audio/ogg"),
        "pdf" => ("document", "application/pdf"),
        "mp4" => ("video", "video/mp4"),
        "webm" => ("video", "video/webm"),
        _ => return None,
    })
}

/// POST /api/materials/upload — multipart: `title`, opsional `class_id`, `file`.
pub async fn upload(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    if !is_manager(&claims.role) {
        return StatusCode::FORBIDDEN.into_response();
    }

    let Some(storage) = state.storage.clone() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Penyimpanan file (RustFS) belum dikonfigurasi di server. Gunakan opsi Tautan, atau hubungi admin teknis.",
        )
            .into_response();
    };

    let mut title = String::new();
    let mut class_id: Option<i64> = None;
    // Berkasnya ada di DISK, bukan di RAM: materi bisa 100 MB (rekaman kajian,
    // pdf kitab), dan menahannya di memori sampai PUT ke RustFS selesai adalah
    // cara paling cepat menghabiskan RAM VPS. Lihat `web::multipart`.
    let mut berkas: Option<crate::web::multipart::BerkasSementara> = None;
    let mut filename = String::new();

    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            "title" => title = field.text().await.unwrap_or_default(),
            "class_id" => {
                let s = field.text().await.unwrap_or_default();
                class_id = s.trim().parse::<i64>().ok().filter(|v| *v > 0);
            }
            "file" => {
                filename = field.file_name().unwrap_or_default().to_string();
                match crate::web::multipart::terima_berkas(
                    field,
                    crate::web::limits::MATERIAL_MAX,
                    "maks 100MB",
                )
                .await
                {
                    Ok(b) => berkas = Some(b),
                    Err(resp) => return resp,
                }
            }
            _ => {}
        }
    }

    let title = title.trim();
    if title.is_empty() {
        return (StatusCode::BAD_REQUEST, "Judul materi wajib diisi.").into_response();
    }
    let Some(berkas) = berkas else {
        return (StatusCode::BAD_REQUEST, "File wajib diunggah.").into_response();
    };
    // Batas ATAS sudah dijaga saat menulis (request diputus di tengah aliran);
    // yang tersisa diperiksa di sini hanya berkas kosong.
    if berkas.ukuran == 0 {
        return (StatusCode::BAD_REQUEST, "Ukuran file tidak valid (maks 100MB).").into_response();
    }
    let Some((kind, content_type)) = classify(&filename) else {
        return (
            StatusCode::BAD_REQUEST,
            "Jenis file tidak didukung (gunakan mp3/wav/ogg, pdf, atau mp4/webm).",
        )
            .into_response();
    };
    // Ekstensi hanya nama pilihan pengunggah — isinya yang menentukan. Tanpa cek
    // ini, berkas apa pun bisa dinamai `.pdf` lalu tersimpan berlabel
    // `application/pdf` dan disajikan kembali dengan label itu. Yang dibaca
    // cukup byte pertamanya, yang memang ditahan `terima_berkas` di memori.
    if !berkas.cocok(content_type) {
        return (
            StatusCode::BAD_REQUEST,
            "Isi file tidak cocok dengan ekstensinya.",
        )
            .into_response();
    }

    let ext = crate::web::filetype::ext_for(content_type);
    let slug: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    // Prefix `ppm/` dibuang: bucket sudah "ppm" (dulu jadi `/ppm/ppm/materials/...`).
    let key = format!("materials/{}-{}.{}", slug, chrono::Utc::now().timestamp(), ext);

    let size = berkas.ukuran as i64;
    // Streaming dari disk — berkasnya tak pernah dimuat utuh ke RAM, baik saat
    // diterima maupun saat diteruskan.
    let url = match storage.upload_file(berkas.path(), &key, content_type).await {
        Ok(u) => u,
        Err(e) => {
            crate::service::telegram::report_error(502, "Materials upload", e.to_string());
            return (StatusCode::BAD_GATEWAY, format!("Upload gagal: {e}")).into_response();
        }
    };

    match crate::repository::insert_material(
        &state.pool, class_id, claims.user_id, title, kind, &url, Some(content_type), Some(size),
    )
    .await
    {
        Ok(id) => (StatusCode::OK, id.to_string()).into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Materials insert", e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()
        }
    }
}
