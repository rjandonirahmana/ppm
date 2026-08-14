//! web/export.rs — Unduh laporan sebagai PDF/Excel (axum murni, di luar
//! server-fn — server function tak cocok utk balas berkas biner). Auth dari
//! cookie ppm_token (pola sama live_audio::auth). Bentuk laporan mengikuti
//! peran, PERSIS logika /laporan (LaporanPage): reuse service::laporan &
//! service::dashboard::analisis — TIDAK ada query DB baru.

use std::sync::Arc;

use axum::extract::Query;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use serde::Deserialize;

use crate::models::{Claims, OutsideRow, SessionUser};
use crate::service::export::{admin_doc, guru_doc, ortu_doc, render_pdf, render_xlsx, santri_doc, ReportDoc};
use crate::service::fmt::fmt_dt_full;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct ExportQ {
    /// "pdf" | "xlsx"
    pub format: String,
}

async fn build_doc(state: &AppState, claims: &Claims) -> Result<ReportDoc, StatusCode> {
    let now = fmt_dt_full(chrono::Utc::now());
    // Cocokkan lewat role_satisfies, JANGAN string mentah: `ketua` setara admin
    // dan `santri_finance` setara santri (models::role_satisfies). Dengan match
    // mentah keduanya jatuh ke `_` → 403, padahal berhak mengekspor.
    let role = if crate::models::role_satisfies(&claims.role, &["admin"]) {
        "admin"
    } else if crate::models::role_satisfies(&claims.role, &["santri"]) {
        "santri"
    } else {
        claims.role.as_str()
    };
    match role {
        "admin" => {
            let (admin, outside) = tokio::join!(
                crate::service::laporan::laporan_admin(&state.pool),
                crate::repository::students_outside(&state.pool, 30),
            );
            let admin = admin.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let outside = outside.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let outside: Vec<OutsideRow> = outside
                .into_iter()
                .map(|r| OutsideRow {
                    user_id: r.user_id,
                    name: r.name,
                    nis: r.nis.unwrap_or_else(|| "-".into()),
                    class_name: r.class_name.unwrap_or_else(|| "-".into()),
                    since_label: r.gate_at.map(fmt_dt_full).unwrap_or_else(|| "-".into()),
                })
                .collect();
            Ok(admin_doc(&admin, &outside, now))
        }
        "teacher" | "dewan_guru" => {
            let teacher_id = (claims.role == "teacher").then_some(claims.user_id);
            let (analisis, extra) = tokio::join!(
                crate::service::dashboard::analisis(&state.pool, &claims.name, teacher_id),
                crate::service::laporan::laporan_guru_extra(&state.pool),
            );
            let analisis = analisis.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let extra = extra.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(guru_doc(&analisis, &extra, now))
        }
        "parent" => {
            let d = crate::service::laporan::laporan_ortu(&state.pool, claims.user_id, None)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(ortu_doc(&d, now))
        }
        "santri" => {
            let sess: SessionUser = claims.clone().into();
            let d = crate::service::laporan::laporan_santri(&state.pool, &sess)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(santri_doc(&d, now))
        }
        _ => Err(StatusCode::FORBIDDEN),
    }
}

/// GET /api/export/laporan?format=pdf|xlsx
pub async fn download(
    Extension(state): Extension<Arc<AppState>>,
    Query(q): Query<ExportQ>,
    headers: HeaderMap,
) -> Response {
    let claims = match super::live_audio::auth(&state, &headers) {
        Ok(c) => c,
        Err(s) => return s.into_response(),
    };
    let doc = match build_doc(&state, &claims).await {
        Ok(d) => d,
        Err(s) => return s.into_response(),
    };

    // Penyusunan PDF/XLSX murni CPU dan sepenuhnya sinkron: menata ratusan baris
    // laporan institusi bisa memakan ratusan milidetik sampai beberapa detik.
    // Dijalankan langsung di sini, ia MENAHAN worker Tokio selama itu — di VPS
    // 2 CPU artinya request lain (SSR halaman, absensi, siaran) ikut membeku
    // hanya karena seorang admin mengunduh laporan. `spawn_blocking` memindahkan
    // pekerjaan itu ke kolam thread khusus; alasannya sama persis dengan yang
    // sudah dipakai untuk bcrypt di service/auth.rs.
    let format = q.format;
    let rendered = tokio::task::spawn_blocking(move || match format.as_str() {
        "pdf" => Some(Ok(("application/pdf", "pdf", render_pdf(&doc)))),
        "xlsx" => Some(render_xlsx(&doc).map(|b| {
            (
                "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
                "xlsx",
                b,
            )
        })),
        _ => None,
    })
    .await;

    let (content_type, ext, bytes) = match rendered {
        Ok(Some(Ok(v))) => v,
        Ok(None) => return StatusCode::BAD_REQUEST.into_response(),
        Ok(Some(Err(e))) => {
            tracing::error!("render laporan gagal: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
        // JoinError = task panik. Dulu panik di dalam render menjatuhkan seluruh
        // request tanpa jejak; sekarang tercatat dan terbalas 500 yang rapi.
        Err(e) => {
            tracing::error!("task render laporan panik: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE.as_str(), content_type.to_string()),
            (
                header::CONTENT_DISPOSITION.as_str(),
                format!("attachment; filename=\"laporan-ppm-afm.{ext}\""),
            ),
        ],
        bytes,
    )
        .into_response()
}
