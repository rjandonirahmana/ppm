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

/// Baca isian `items` (JSON) atau pasangan `amount`/`student_id` gaya lama.
///
/// Bentuk JSON dipakai sejak orang tua boleh membayar beberapa anak dalam satu
/// transfer: `[{"student_id":12,"amount":500000},{"student_id":15,"amount":500000}]`.
/// Nominal disaring ke digit saja — pengguna mengetik "500.000" karena itulah
/// yang terlihat di aplikasi banknya.
fn urai_items(json: &str) -> Option<Vec<crate::repository::PengajuanAnak>> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let arr = v.as_array()?;
    let mut out = Vec::with_capacity(arr.len());
    for it in arr {
        let student_id = it.get("student_id")?.as_i64()?;
        let amount = match it.get("amount")? {
            serde_json::Value::Number(n) => n.as_i64()?,
            serde_json::Value::String(s) => {
                s.chars().filter(|c| c.is_ascii_digit()).collect::<String>().parse().ok()?
            }
            _ => return None,
        };
        out.push(crate::repository::PengajuanAnak { student_id, amount });
    }
    Some(out)
}

/// POST /api/bills/request — santri / orang tua MENGAJUKAN pembayaran.
///
/// multipart: `file` (foto bukti transfer, wajib), `note` (opsional), dan
/// salah satu dari:
///   • `items` — JSON `[{student_id, amount}, …]` untuk satu transfer yang
///     mencakup BEBERAPA anak (orang tua dengan lebih dari satu anak);
///   • `amount` + `student_id` — bentuk tunggal (santri untuk dirinya sendiri).
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
    let mut items: Option<Vec<crate::repository::PengajuanAnak>> = None;
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
            "items" => {
                let raw = field.text().await.unwrap_or_default();
                match urai_items(&raw) {
                    Some(v) => items = Some(v),
                    // Bentuknya rusak = permintaan yang tak bisa ditafsirkan.
                    // Jatuh diam-diam ke jalur tunggal akan menyimpan kiriman
                    // untuk SATU anak padahal keluarga memilih dua.
                    None => {
                        return (StatusCode::BAD_REQUEST, "Daftar santri tidak terbaca.")
                            .into_response()
                    }
                }
            }
            "note" => catatan = field.text().await.unwrap_or_default(),
            "file" | "image" => match field.bytes().await {
                Ok(b) => file_bytes = Some(b.to_vec()),
                Err(e) => return (StatusCode::BAD_REQUEST, e.to_string()).into_response(),
            },
            _ => {}
        }
    }

    // Bentuk tunggal disusun jadi daftar satu isi, supaya sisa handler ini
    // hanya punya SATU jalur — satu tempat validasi, satu tempat penyimpanan.
    let items = items.unwrap_or_else(|| {
        vec![crate::repository::PengajuanAnak {
            student_id: if student_id > 0 { student_id } else { claims.user_id },
            amount,
        }]
    });

    // Seluruh isi kiriman diperiksa SEBELUM fotonya diunggah: menolak setelah
    // unggahan selesai berarti membuang kuota keluarga untuk sesuatu yang sudah
    // bisa diketahui sejak awal — dan meninggalkan berkas yatim di penyimpanan.
    if let Err(e) =
        crate::service::finance::periksa_kiriman(&state.pool, claims.user_id, &items).await
    {
        let pesan = pesan_pengguna(&e);
        // Galat yang BUKAN pesan-untuk-pengguna berarti kegagalan sistem
        // (DB tak terjangkau), bukan masukan yang salah.
        let kode = if pesan == "Data tidak valid." {
            crate::service::telegram::report_error(500, "Periksa kiriman bayar", e.to_string());
            StatusCode::INTERNAL_SERVER_ERROR
        } else {
            StatusCode::BAD_REQUEST
        };
        return (kode, pesan).into_response();
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

    // Satu bukti untuk seluruh kiriman — buktinya memang satu lembar transfer.
    let key = format!(
        "bills/{}-{}.{}",
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0),
        claims.user_id,
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
    match crate::repository::ajukan_pembayaran_batch(
        &state.pool,
        &items,
        claims.user_id,
        &url,
        &catatan,
    )
    .await
    {
        Ok(ids) => (
            StatusCode::OK,
            Json(json!({ "ids": ids, "jumlah": ids.len(), "url": url })),
        )
            .into_response(),
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
