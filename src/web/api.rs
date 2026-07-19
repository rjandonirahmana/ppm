//! web/api.rs — Server functions (/api-fn). Lapisan tipis: ekstrak state/sesi
//! → panggil `service::*` → kembalikan DTO `models::*`.
//!
//! Sesi via cookie HttpOnly `ppm_token` (JWT). Pola sama proyek e-ticketing.

use leptos::prelude::*;

use crate::models::{
    AnalisisData, ChildRiwayat, ConnRequest, IzinData, KelasData, KelasDetail, PamongData,
    ParentHome, ParentPermitItem, PoinData, ProfilData, RiwayatData, SantriHome, SessionUser,
    SessionsData, StafHome, StudentSearchItem, StudentsData,
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
    let sess = require_roles(&["santri", "teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::list_for(&state.pool, &sess)
        .await
        .map_err(err)
}

/// Detail sesi (staf): absensi anggota + chat + rekaman.
#[server(GetSessionDetail, "/api-fn")]
pub async fn session_detail_data(
    session_id: i64,
) -> Result<crate::models::SessionDetailData, ServerFnError> {
    let sess = require_roles(&["teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::detail_for(&state.pool, &sess, session_id)
        .await
        .map_err(err)
}

/// Ruang sesi live: info + chat (staf & santri peserta kelas).
#[server(GetSessionLive, "/api-fn")]
pub async fn session_live_data(
    session_id: i64,
) -> Result<crate::models::SessionLiveData, ServerFnError> {
    let sess = require_roles(&["santri", "teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::live_for(&state.pool, &sess, session_id)
        .await
        .map_err(err)
}

/// Kirim pesan chat sesi.
#[server(PostSessionChat, "/api-fn")]
pub async fn post_session_chat(session_id: i64, message: String) -> Result<(), ServerFnError> {
    let sess = require_roles(&["santri", "teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::post_chat(&state.pool, &sess, session_id, &message)
        .await
        .map_err(err)
}

/// Mulai/akhiri sesi live (staf).
#[server(SetSessionLive, "/api-fn")]
pub async fn set_session_live(session_id: i64, start: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::set_live(&state.pool, &sess, session_id, start)
        .await
        .map_err(err)
}

/// Tandai santri HADIR manual pada sebuah sesi (staf).
#[server(MarkSessionPresent, "/api-fn")]
pub async fn mark_session_present(
    session_id: i64,
    student_id: i64,
) -> Result<(), ServerFnError> {
    let sess = require_roles(&["teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::mark_present(&state.pool, &sess, session_id, student_id)
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

/// Antrean verifikasi TAHAP 2 (dewan guru/admin) — final.
#[server(GetVerifyData, "/api-fn")]
pub async fn verify_data() -> Result<PamongData, ServerFnError> {
    require_roles(&["dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::attendance::verify_data(&state.pool)
        .await
        .map_err(err)
}

/// Verifikasi final satu absensi (tahap 2).
#[server(DecideVerify, "/api-fn")]
pub async fn decide_verify(att_id: i64, approve: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::attendance::decide_verify(&state.pool, att_id, sess.id, approve)
        .await
        .map_err(err)?;
    Ok(())
}

// ── Manajemen Kelas (admin/dewan guru/pamong) ─────────────────────────────────────

#[cfg(feature = "ssr")]
const KELAS_ROLES: &[&str] = &["admin", "dewan_guru", "supervisor", "teacher"];

/// Daftar kelas + statistik (/kelas).
#[server(GetKelasList, "/api-fn")]
pub async fn kelas_list() -> Result<KelasData, ServerFnError> {
    let sess = require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::kelas_list(&state.pool, &sess.role)
        .await
        .map_err(err)
}

/// Detail kelas (/kelas/:id) — anggota, jadwal, sesi, opsi form.
#[server(GetKelasDetail, "/api-fn")]
pub async fn kelas_detail(class_id: i64) -> Result<KelasDetail, ServerFnError> {
    let sess = require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::kelas_detail(&state.pool, &sess.role, class_id)
        .await
        .map_err(err)
}

/// Buat kelas baru (nama + kategori fleksibel).
#[server(CreateClass, "/api-fn")]
pub async fn create_class_action(
    name: String,
    category: String,
    description: String,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::create_class(&state.pool, &name, &category, &description)
        .await
        .map_err(err)
}

/// Ubah kelas (nama + kategori) — Edit Detail Kelas.
#[server(UpdateClass, "/api-fn")]
pub async fn update_class_action(
    class_id: i64,
    name: String,
    category: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::update_class(&state.pool, class_id, &name, &category)
        .await
        .map_err(err)
}

/// Kategori kelas yang sudah dipakai (dropdown + boleh ketik baru).
#[server(GetCategories, "/api-fn")]
pub async fn class_categories() -> Result<Vec<String>, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::categories(&state.pool)
        .await
        .map_err(err)
}

/// Buat jadwal baru untuk sebuah kelas.
#[server(CreateSchedule, "/api-fn")]
pub async fn create_schedule_action(
    class_id: i64,
    title: String,
    start_time: String,
    end_time: String,
    limit_time: String,
    recurrence: String,
    start_date: String,
    end_date: String,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::create_schedule(
        &state.pool, class_id, &title, &start_time, &end_time, &limit_time, &recurrence,
        &start_date, &end_date,
    )
    .await
    .map_err(err)
}

/// Buat sesi baru untuk sebuah kelas.
#[server(CreateSession, "/api-fn")]
pub async fn create_session_action(
    class_id: i64,
    schedule_id: i64,
    teacher_id: i64,
    title: String,
    session_date: String,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::create_session(
        &state.pool,
        class_id,
        Some(schedule_id),
        Some(teacher_id),
        &title,
        &session_date,
    )
    .await
    .map_err(err)
}

/// Tambah santri ke kelas (pada jadwal terpilih).
#[server(AddMember, "/api-fn")]
pub async fn add_member_action(
    class_id: i64,
    schedule_id: i64,
    student_id: i64,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::add_member(&state.pool, class_id, schedule_id, student_id)
        .await
        .map_err(err)
}

/// Ubah jadwal.
#[server(UpdateSchedule, "/api-fn")]
pub async fn update_schedule_action(
    schedule_id: i64,
    title: String,
    start_time: String,
    end_time: String,
    limit_time: String,
    recurrence: String,
    start_date: String,
    end_date: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::update_schedule(
        &state.pool, schedule_id, &title, &start_time, &end_time, &limit_time, &recurrence,
        &start_date, &end_date,
    )
    .await
    .map_err(err)
}

/// Hapus jadwal.
#[server(DeleteSchedule, "/api-fn")]
pub async fn delete_schedule_action(schedule_id: i64) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::delete_schedule(&state.pool, schedule_id)
        .await
        .map_err(err)
}

/// Generate sesi satu bulan dari sebuah jadwal (materialisasi). Return jumlah baru.
#[server(GenerateMonthSessions, "/api-fn")]
pub async fn generate_month_action(
    schedule_id: i64,
    year: i32,
    month: u32,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::generate_month_sessions(&state.pool, schedule_id, year, month)
        .await
        .map_err(err)
}

/// Keluarkan santri dari kelas.
#[server(RemoveMember, "/api-fn")]
pub async fn remove_member_action(class_id: i64, student_id: i64) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::remove_member(&state.pool, class_id, student_id)
        .await
        .map_err(err)
}

/// Pasang/ubah pengajar sebuah sesi (0 = kosongkan).
#[server(SetSessionTeacher, "/api-fn")]
pub async fn set_session_teacher_action(session_id: i64, teacher_id: i64) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_session_teacher(&state.pool, session_id, teacher_id)
        .await
        .map_err(err)
}

/// Tandai sesi libur / aktif kembali.
#[server(SetSessionLibur, "/api-fn")]
pub async fn set_session_libur_action(session_id: i64, libur: bool) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_session_libur(&state.pool, session_id, libur)
        .await
        .map_err(err)
}

/// Cari santri (nama/NIS) untuk ditambahkan ke kelas.
#[server(StaffSearchStudents, "/api-fn")]
pub async fn staff_search_students(q: String) -> Result<Vec<StudentSearchItem>, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::search_students(&state.pool, &q)
        .await
        .map_err(err)
}

/// Halaman Students: daftar santri + antrean verifikasi sesuai peran.
#[server(GetStudentsData, "/api-fn")]
pub async fn students_data() -> Result<StudentsData, ServerFnError> {
    let sess = require_roles(&["admin", "dewan_guru", "supervisor", "teacher"]).await?;
    let state = app_state().await?;
    crate::service::kelas::students_data(&state.pool, &sess)
        .await
        .map_err(err)
}

// ── Sisi STAF / GURU / DEWAN GURU ────────────────────────────────────────────────

/// Dashboard staf (/staf) — statistik hari ini, sesi live, kehadiran terbaru.
#[server(GetStafHome, "/api-fn")]
pub async fn staf_home_data() -> Result<StafHome, ServerFnError> {
    let sess = require_roles(&["admin", "dewan_guru", "supervisor"]).await?;
    let state = app_state().await?;
    crate::service::dashboard::staf_home(&state.pool, &sess.name)
        .await
        .map_err(err)
}

/// Dashboard analisis (/guru → cakupan kelas sendiri, /dewan-guru → semua kelas).
#[server(GetAnalisis, "/api-fn")]
pub async fn analisis_data() -> Result<AnalisisData, ServerFnError> {
    let sess = require_roles(&["teacher", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    // Guru biasa: dibatasi ke kelas yang diampu sendiri. Dewan guru/admin: semua.
    let teacher_id = (sess.role == "teacher").then_some(sess.id);
    crate::service::dashboard::analisis(&state.pool, &sess.name, teacher_id)
        .await
        .map_err(err)
}

/// Papan poin santri (/poin, /poin-dewan). Dewan guru/admin melihat & mengelola
/// SEMUA santri; guru/pamong hanya santri di kelas yang mereka ampu.
#[server(GetPoinData, "/api-fn")]
pub async fn poin_data_action() -> Result<PoinData, ServerFnError> {
    let sess = require_roles(&["admin", "dewan_guru", "teacher", "supervisor"]).await?;
    let state = app_state().await?;
    let can_adjust = matches!(sess.role.as_str(), "admin" | "dewan_guru");
    let teacher_id = (sess.role == "teacher").then_some(sess.id);
    crate::service::dashboard::poin_data(&state.pool, teacher_id, can_adjust)
        .await
        .map_err(err)
}

/// Tambah/kurangi poin santri secara manual (dewan guru/admin saja).
#[server(AdjustPoints, "/api-fn")]
pub async fn adjust_points_action(student_id: i64, delta: i32, reason: String) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    let reason = if reason.trim().is_empty() { "Penyesuaian manual".to_string() } else { reason };
    crate::repository::adjust_points(&state.pool, student_id, delta, &reason, sess.id)
        .await
        .map_err(err)
}
