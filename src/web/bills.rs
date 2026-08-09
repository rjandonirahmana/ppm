//! web/bills.rs — Santri unggah bukti bayar tagihan (migrasi 37). Handler axum
//! multipart (di luar server-fn). Auth cookie; hanya boleh set bukti tagihan
//! MILIK SENDIRI (guard di repo::set_proof). Balas JSON `{ url }`.

use std::sync::Arc;

use axum::extract::Multipart;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde_json::json;

use crate::state::AppState;

pub async fn upload_proof(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };

    let Some(storage) = state.storage.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Penyimpanan (RustFS) belum aktif.").into_response();
    };

    let mut bill_id: i64 = 0;
    let mut file_bytes: Option<Vec<u8>> = None;
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            "bill_id" => {
                bill_id = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0);
            }
            "file" | "image" => match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            },
            _ => {}
        }
    }

    if bill_id <= 0 {
        return (StatusCode::BAD_REQUEST, "bill_id wajib.").into_response();
    }
    let Some(bytes) = file_bytes.filter(|b| !b.is_empty() && b.len() <= crate::web::limits::IMAGE_MAX) else {
        return (StatusCode::BAD_REQUEST, "File bukti tidak valid (maks 10MB).").into_response();
    };

    // Dulu APA PUN disimpan sebagai `.jpg` berlabel `image/jpeg` tanpa
    // diperiksa — bukti bayar PNG pun ikut salah label, dan berkas yang sama
    // sekali bukan gambar tetap diterima. Sekarang tipenya dibaca dari isi
    // berkas, lalu label & ekstensinya mengikuti hasil itu.
    let Some(content_type) = crate::web::filetype::sniff_image(&bytes) else {
        return (
            StatusCode::BAD_REQUEST,
            "Bukti bayar harus berupa gambar (jpg/png/webp/gif).",
        )
            .into_response();
    };

    let key = format!(
        "bills/{}-{}.{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        claims.user_id,
        crate::web::filetype::ext_for(content_type)
    );
    let url = match storage.upload_bytes(bytes, &key, content_type).await {
        Ok(u) => u,
        Err(e) => {
            crate::service::telegram::report_error(502, "Bill proof upload", e.to_string());
            return (StatusCode::BAD_GATEWAY, format!("Upload gagal: {e}")).into_response();
        }
    };

    // Guard kepemilikan di query (user_id = pengunggah).
    match crate::repository::set_proof(&state.pool, bill_id, claims.user_id, &url).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "url": url }))).into_response(),
        Ok(false) => (StatusCode::FORBIDDEN, "Bukan tagihan Anda.").into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Bill proof simpan", e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, "Gagal menyimpan bukti, coba lagi.")
                .into_response()
        }
    }
}

/// POST /api/bills/request — santri / orang tua MENGAJUKAN pembayaran.
///
/// multipart: `amount` (rupiah, wajib), `file` (foto bukti transfer, wajib),
/// `student_id` (opsional; kosong = diri sendiri), `note` (opsional).
///
/// SATU request, bukan dua. Memisahkan "buat baris" dan "unggah bukti" akan
/// meninggalkan pengajuan tanpa bukti setiap kali unggahannya gagal di tengah —
/// dan baris seperti itu muncul di antrean verifikator sebagai kiriman yang tak
/// bisa diapa-apakan, sementara keluarga mengira sudah selesai.
///
/// Handler axum, bukan server fn: server fn Leptos tak bisa menerima berkas
/// multipart (alasan yang sama dengan `upload_proof` dan materials).
pub async fn request_payment(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    mut form: Multipart,
) -> Response {
    let claims = match crate::web::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };

    let Some(storage) = state.storage.clone() else {
        return (StatusCode::SERVICE_UNAVAILABLE, "Penyimpanan (RustFS) belum aktif.")
            .into_response();
    };

    let mut amount: i64 = 0;
    let mut student_id: i64 = 0;
    let mut catatan = String::new();
    let mut file_bytes: Option<Vec<u8>> = None;
    loop {
        let field = match form.next_field().await {
            Ok(Some(f)) => f,
            Ok(None) => break,
            Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
        };
        match field.name().unwrap_or_default() {
            // Titik & spasi dibuang: pengguna mengetik "500.000" karena itulah
            // yang terlihat di aplikasi bank, dan menolaknya sebagai "bukan
            // angka" adalah menyalahkan orang atas kebiasaan yang wajar.
            "amount" => {
                let t = field.text().await.unwrap_or_default();
                let bersih: String = t.chars().filter(|c| c.is_ascii_digit()).collect();
                amount = bersih.parse().unwrap_or(0);
            }
            "student_id" => {
                student_id = field.text().await.unwrap_or_default().trim().parse().unwrap_or(0);
            }
            "note" => catatan = field.text().await.unwrap_or_default(),
            "file" | "image" => match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            },
            _ => {}
        }
    }

    let student_id = if student_id > 0 { student_id } else { claims.user_id };
    if let Err(e) = crate::service::finance::periksa_nominal(amount) {
        return (StatusCode::BAD_REQUEST, pesan_pengguna(&e)).into_response();
    }
    match crate::service::finance::boleh_mengajukan(&state.pool, claims.user_id, student_id).await {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::FORBIDDEN,
                "Anda tidak terhubung dengan santri ini. Hubungi admin untuk menautkan akun.",
            )
                .into_response()
        }
        Err(e) => {
            crate::service::telegram::report_error(500, "Cek koneksi ortu", e.to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, "Gagal memeriksa akses, coba lagi.")
                .into_response();
        }
    }

    let Some(bytes) =
        file_bytes.filter(|b| !b.is_empty() && b.len() <= crate::web::limits::IMAGE_MAX)
    else {
        return (
            StatusCode::BAD_REQUEST,
            "Foto bukti transfer wajib diunggah (maks 10MB).",
        )
            .into_response();
    };
    let Some(content_type) = crate::web::filetype::sniff_image(&bytes) else {
        return (
            StatusCode::BAD_REQUEST,
            "Bukti transfer harus berupa foto (jpg/png/webp). Tangkapan layar dari HP \
             biasanya sudah sesuai.",
        )
            .into_response();
    };

    let key = format!(
        "bills/{}-{}.{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        student_id,
        crate::web::filetype::ext_for(content_type)
    );
    let url = match storage.upload_bytes(bytes, &key, content_type).await {
        Ok(u) => u,
        Err(e) => {
            crate::service::telegram::report_error(502, "Bukti pengajuan upload", e.to_string());
            return (
                StatusCode::BAD_GATEWAY,
                "Gagal mengunggah foto — periksa koneksi, lalu coba lagi.",
            )
                .into_response();
        }
    };

    let catatan = catatan.trim().chars().take(300).collect::<String>();
    match crate::repository::ajukan_pembayaran(
        &state.pool,
        student_id,
        claims.user_id,
        amount,
        &url,
        &catatan,
    )
    .await
    {
        Ok(id) => (StatusCode::OK, Json(json!({ "id": id, "url": url }))).into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Simpan pengajuan bayar", e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, "Gagal menyimpan pengajuan, coba lagi.")
                .into_response()
        }
    }
}

/// Ambil pesan yang memang ditujukan untuk pengguna dari galat service;
/// selain itu balas kalimat umum (pola sama `web/api.rs::err`).
fn pesan_pengguna(e: &anyhow::Error) -> String {
    e.downcast_ref::<crate::service::UserError>()
        .map(|u| u.0.clone())
        .unwrap_or_else(|| "Data tidak valid.".to_string())
}
