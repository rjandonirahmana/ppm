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
    match claims.role.as_str() {
        "admin" | "supervisor" => {
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

    match q.format.as_str() {
        "pdf" => {
            let bytes = render_pdf(&doc);
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE.as_str(), "application/pdf".to_string()),
                    (
                        header::CONTENT_DISPOSITION.as_str(),
                        "attachment; filename=\"laporan-ppm-afm.pdf\"".to_string(),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        "xlsx" => match render_xlsx(&doc) {
            Ok(bytes) => (
                StatusCode::OK,
                [
                    (
                        header::CONTENT_TYPE.as_str(),
                        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet".to_string(),
                    ),
                    (
                        header::CONTENT_DISPOSITION.as_str(),
                        "attachment; filename=\"laporan-ppm-afm.xlsx\"".to_string(),
                    ),
                ],
                bytes,
            )
                .into_response(),
            Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
        },
        _ => StatusCode::BAD_REQUEST.into_response(),
    }
}
