//! web/ics.rs — Penyajian langganan kalender (`GET /kalender.ics`).
//!
//! Handler axum murni, DI LUAR server-fn dan di luar cookie sesi. Itu keharusan
//! bentuknya: yang mengambil berkas ini adalah server Google (atau aplikasi
//! kalender), bukan peramban santri — tak ada cookie yang ikut terkirim, dan
//! tak ada halaman login yang bisa ditampilkan. Karena itu wewenangnya dibawa
//! oleh token di dalam URL-nya sendiri.
//!
//! Konsekuensi yang harus disadari: siapa pun yang memegang URL itu bisa
//! membaca jadwal orang tersebut, selamanya, tanpa login. Isinya jadwal kelas —
//! bukan nilai, poin, atau keuangan — dan itulah batas yang sengaja dijaga:
//! berkas ini tak boleh berkembang memuat apa pun yang lebih pribadi.

use std::sync::Arc;

use axum::extract::{Query, Extension};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::state::AppState;

#[derive(Deserialize)]
pub struct Params {
    /// Id pengguna pemilik jadwal.
    u: Option<i64>,
    /// Token langganan (lihat `service::ics::token_langganan`).
    t: Option<String>,
}

/// GET /kalender.ics?u=<id>&t=<token>
pub async fn feed(
    Extension(state): Extension<Arc<AppState>>,
    Query(p): Query<Params>,
) -> Response {
    let (Some(user_id), Some(token)) = (p.u, p.t) else {
        return (StatusCode::BAD_REQUEST, "Alamat langganan tidak lengkap.").into_response();
    };
    if user_id <= 0 || !crate::service::ics::token_cocok(&state.jwt_secret, user_id, &token) {
        // 404, bukan 403: membedakan "token salah" dari "pengguna tak ada"
        // memberi tahu penebak bahwa id yang ia coba memang ada.
        return (StatusCode::NOT_FOUND, "Langganan tidak ditemukan.").into_response();
    }

    // Identitas dibaca SEGAR dari DB tiap permintaan — bukan dititipkan di URL.
    // Akun yang dinonaktifkan berhenti menyajikan jadwal pada tarikan
    // berikutnya, tanpa perlu mencabut tautannya satu per satu.
    let user = match crate::repository::session_user_aktif(&state.pool, user_id).await {
        Ok(Some(u)) => u,
        Ok(None) => return (StatusCode::NOT_FOUND, "Langganan tidak ditemukan.").into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Feed kalender", e.to_string());
            return (StatusCode::INTERNAL_SERVER_ERROR, "Gagal menyusun kalender.")
                .into_response();
        }
    };

    match crate::service::ics::bangun_ics(&state.pool, &state.jwt_secret, &user).await {
        Ok(body) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "text/calendar; charset=utf-8"),
                // Nama berkas dipakai klien yang mengunduh alih-alih berlangganan.
                (
                    header::CONTENT_DISPOSITION,
                    "inline; filename=\"jadwal-afm-smart.ics\"",
                ),
                // Jangan di-cache proxy: jadwal berubah, dan tarikan berikutnya
                // harus melihat perubahannya.
                (header::CACHE_CONTROL, "no-store, max-age=0"),
            ],
            body,
        )
            .into_response(),
        Err(e) => {
            crate::service::telegram::report_error(500, "Susun ICS", e.to_string());
            (StatusCode::INTERNAL_SERVER_ERROR, "Gagal menyusun kalender.").into_response()
        }
    }
}
