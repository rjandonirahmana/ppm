//! web/api.rs — Server functions (/api-fn). Lapisan tipis: ekstrak state/sesi
//! → panggil `service::*` → kembalikan DTO `models::*`.
//!
//! Sesi via cookie HttpOnly `ppm_token` (JWT). Pola sama proyek e-ticketing.

use leptos::prelude::*;

use crate::models::{
    ChildRiwayat, ConnRequest, IzinData, PamongData, ParentHome, ParentPermitItem, ProfilData,
    RiwayatData, SantriHome, SessionUser, SessionsData, StudentSearchItem,
};

// ── Helper server-only ─────────────────────────────────────────────────────────

#[cfg(feature = "ssr")]
mod ssr_helpers {
    use crate::models::SessionUser;
    use crate::state::AppState;
    use leptos::prelude::*;
    use std::sync::Arc;

    pub async fn app_state() -> Result<Arc<AppState>, ServerFnError> {
        use axum::Extension;
        leptos_axum::extract::<Extension<Arc<AppState>>>()
            .await
            .map(|e| e.0)
            .map_err(|e| ServerFnError::new(format!("AppState unavailable: {e}")))
    }

    /// Baca token dari header Cookie.
    pub async fn auth_token() -> Option<String> {
        use axum::http::{header::COOKIE, HeaderMap};
        let headers: HeaderMap = leptos_axum::extract().await.ok()?;
        let cookie = headers.get(COOKIE)?.to_str().ok()?;
        cookie.split(';').map(|p| p.trim()).find_map(|p| {
            p.strip_prefix(&format!("{}=", crate::auth::COOKIE_NAME))
                .filter(|v| !v.is_empty())
                .map(String::from)
        })
    }

    pub fn set_auth_cookie(token: &str) {
        use axum::http::{header::SET_COOKIE, HeaderValue};
        let resp = expect_context::<leptos_axum::ResponseOptions>();
        let v = format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}",
            crate::auth::COOKIE_NAME,
            crate::auth::SESSION_SECS
        );
        if let Ok(hv) = HeaderValue::from_str(&v) {
            resp.append_header(SET_COOKIE, hv);
        }
    }

    pub fn clear_auth_cookie() {
        use axum::http::{header::SET_COOKIE, HeaderValue};
        let resp = expect_context::<leptos_axum::ResponseOptions>();
        // Max-Age=0 + Expires epoch (dua-duanya) — beberapa browser/proxy hanya
        // menghormati salah satunya. Pola sama e-ticketing.
        let v = format!(
            "{}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0; \
             Expires=Thu, 01 Jan 1970 00:00:00 GMT",
            crate::auth::COOKIE_NAME
        );
        if let Ok(hv) = HeaderValue::from_str(&v) {
            resp.append_header(SET_COOKIE, hv);
        }
    }

    /// Sesi wajib — Err("unauth") bila belum login/token invalid.
    pub async fn require_session() -> Result<SessionUser, ServerFnError> {
        let state = app_state().await?;
        let token = auth_token()
            .await
            .ok_or_else(|| ServerFnError::new("unauth"))?;
        let claims = state
            .jwt
            .verify(&token)
            .map_err(|_| ServerFnError::new("unauth"))?;
        Ok(claims.into())
    }

    /// Sesi wajib + peran harus salah satu dari `roles`.
    pub async fn require_roles(roles: &[&str]) -> Result<SessionUser, ServerFnError> {
        let s = require_session().await?;
        if roles.contains(&s.role.as_str()) {
            Ok(s)
        } else {
            Err(ServerFnError::new("forbidden"))
        }
    }

    pub fn err<E: std::fmt::Display>(e: E) -> ServerFnError {
        ServerFnError::new(e.to_string())
    }
}

#[cfg(feature = "ssr")]
use ssr_helpers::*;

// ── Server functions ───────────────────────────────────────────────────────────

/// Login: verifikasi kredensial → set cookie sesi → return path redirect per peran.
#[server(LoginAction, "/api-fn")]
pub async fn login_action(login: String, password: String) -> Result<String, ServerFnError> {
    let state = app_state().await?;
    let ok = crate::service::auth::login(&state.pool, &state.jwt, &login, &password)
        .await
        .map_err(err)?;
    set_auth_cookie(&ok.token);
    Ok(ok.redirect)
}

/// Sesi saat ini (None bila belum login). Direkonstruksi murni dari claims
/// JWT — zero query DB (pola sama e-ticketing get_session).
///
/// SLIDING SESSION: bila token valid, token BARU di-sign dan cookie di-set
/// ulang (umur penuh lagi). Pengguna aktif tak pernah diminta login ulang.
#[server(GetSession, "/api-fn")]
pub async fn get_session() -> Result<Option<SessionUser>, ServerFnError> {
    let state = app_state().await?;
    let Some(token) = auth_token().await else {
        return Ok(None);
    };
    let Some(claims) = state.jwt.verify(&token).ok() else {
        // Token rusak/kedaluwarsa → bersihkan agar tak dikirim berulang.
        clear_auth_cookie();
        return Ok(None);
    };
    // Perpanjang sesi (best-effort; gagal sign bukan alasan menolak sesi).
    if let Ok(fresh) = state
        .jwt
        .sign(claims.user_id, &claims.name, &claims.phone, &claims.role)
    {
        set_auth_cookie(&fresh);
    }
    Ok(Some(claims.into()))
}

#[server(LogoutAction, "/api-fn")]
pub async fn logout_action() -> Result<(), ServerFnError> {
    clear_auth_cookie();
    Ok(())
}

/// Data dashboard santri (butuh sesi santri; admin boleh utk inspeksi).
#[server(GetSantriHome, "/api-fn")]
pub async fn santri_home() -> Result<SantriHome, ServerFnError> {
    let sess = require_roles(&["santri", "admin"]).await?;
    let state = app_state().await?;
    crate::service::dashboard::santri_home(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Riwayat kehadiran lengkap santri.
#[server(GetRiwayat, "/api-fn")]
pub async fn riwayat_data() -> Result<RiwayatData, ServerFnError> {
    let sess = require_roles(&["santri", "admin"]).await?;
    let state = app_state().await?;
    crate::service::santri::riwayat(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Data halaman Ajukan Perizinan.
#[server(GetIzinData, "/api-fn")]
pub async fn izin_data() -> Result<IzinData, ServerFnError> {
    let sess = require_roles(&["santri", "admin"]).await?;
    let state = app_state().await?;
    crate::service::santri::izin_data(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Ajukan izin baru (sick|leave, tanggal "YYYY-MM-DD", alasan).
#[server(SubmitPermitAction, "/api-fn")]
pub async fn submit_permit_action(
    kind: String,
    start: String,
    end: String,
    reason: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(&["santri", "admin"]).await?;
    let state = app_state().await?;
    crate::service::santri::submit_permit(&state.pool, sess.id, &kind, &start, &end, &reason)
        .await
        .map_err(err)
}

/// Data profil pengguna login (semua peran).
#[server(GetProfil, "/api-fn")]
pub async fn profil_data() -> Result<ProfilData, ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::santri::profil(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Daftar sesi kelas. Santri → sesi kelasnya sendiri; admin/pamong/dewan guru →
/// SEMUA sesi. Orang tua tidak punya akses.
#[server(GetSessions, "/api-fn")]
pub async fn sessions_list() -> Result<SessionsData, ServerFnError> {
    let sess = require_roles(&["santri", "teacher", "supervisor", "admin"]).await?;
    let state = app_state().await?;
    crate::service::sessions::list_for(&state.pool, &sess)
        .await
        .map_err(err)
}

// ── Sisi ORANG TUA ─────────────────────────────────────────────────────────────

/// Cari santri (nama/NIS) untuk koneksi.
#[server(SearchStudents, "/api-fn")]
pub async fn search_students_action(q: String) -> Result<Vec<StudentSearchItem>, ServerFnError> {
    require_roles(&["parent", "admin"]).await?;
    let state = app_state().await?;
    crate::service::parent::search_students(&state.pool, &q)
        .await
        .map_err(err)
}

/// Kirim permintaan koneksi ke seorang santri (menunggu persetujuan santri).
#[server(RequestConnection, "/api-fn")]
pub async fn request_connection_action(student_id: i64) -> Result<(), ServerFnError> {
    let sess = require_roles(&["parent"]).await?;
    let state = app_state().await?;
    crate::service::parent::request_connection(&state.pool, sess.id, student_id)
        .await
        .map_err(err)
}

/// Beranda orang tua (anak terhubung + pending + pantauan anak terpilih).
#[server(GetParentHome, "/api-fn")]
pub async fn parent_home(child: Option<i64>) -> Result<ParentHome, ServerFnError> {
    let sess = require_roles(&["parent", "admin"]).await?;
    let state = app_state().await?;
    crate::service::parent::parent_home(&state.pool, sess.id, child)
        .await
        .map_err(err)
}

/// Riwayat kehadiran ANAK (guard koneksi di service).
#[server(GetChildRiwayat, "/api-fn")]
pub async fn child_riwayat(child_id: i64) -> Result<ChildRiwayat, ServerFnError> {
    let sess = require_roles(&["parent"]).await?;
    let state = app_state().await?;
    crate::service::parent::child_riwayat(&state.pool, sess.id, child_id)
        .await
        .map_err(err)
}

/// Daftar izin seluruh anak terhubung.
#[server(GetChildrenPermits, "/api-fn")]
pub async fn children_permits() -> Result<Vec<ParentPermitItem>, ServerFnError> {
    let sess = require_roles(&["parent"]).await?;
    let state = app_state().await?;
    crate::service::parent::children_permits(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Ortu mengajukan izin untuk anaknya.
#[server(SubmitChildPermit, "/api-fn")]
pub async fn submit_child_permit_action(
    child_id: i64,
    kind: String,
    start: String,
    end: String,
    reason: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(&["parent"]).await?;
    let state = app_state().await?;
    crate::service::parent::submit_child_permit(
        &state.pool, sess.id, child_id, &kind, &start, &end, &reason,
    )
    .await
    .map_err(err)
}

/// Permintaan koneksi MASUK (sisi santri).
#[server(GetConnRequests, "/api-fn")]
pub async fn connection_requests() -> Result<Vec<ConnRequest>, ServerFnError> {
    let sess = require_roles(&["santri"]).await?;
    let state = app_state().await?;
    crate::service::parent::incoming_requests(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Santri menyetujui/menolak permintaan koneksi orang tua.
#[server(RespondConnRequest, "/api-fn")]
pub async fn respond_connection_action(conn_id: i64, approve: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["santri"]).await?;
    let state = app_state().await?;
    crate::service::parent::respond_request(&state.pool, sess.id, conn_id, approve)
        .await
        .map_err(err)?;
    Ok(())
}

/// Antrean verifikasi pamong (supervisor/teacher/admin).
#[server(GetPamongData, "/api-fn")]
pub async fn pamong_data() -> Result<PamongData, ServerFnError> {
    require_roles(&["supervisor", "teacher", "admin"]).await?;
    let state = app_state().await?;
    crate::service::attendance::pamong_data(&state.pool)
        .await
        .map_err(err)
}

/// Setujui/tolak satu absensi (tahap pamong).
#[server(DecidePamong, "/api-fn")]
pub async fn decide_pamong(att_id: i64, approve: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["supervisor", "teacher", "admin"]).await?;
    let state = app_state().await?;
    crate::service::attendance::decide_pamong(&state.pool, att_id, sess.id, approve)
        .await
        .map_err(err)?;
    Ok(())
}
