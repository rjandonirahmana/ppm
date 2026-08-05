//! web/api.rs — Server functions (/api-fn). Lapisan tipis: ekstrak state/sesi
//! → panggil `service::*` → kembalikan DTO `models::*`.
//!
//! Sesi via cookie HttpOnly `ppm_token` (JWT). Pola sama proyek e-ticketing.

use leptos::prelude::*;

use crate::models::{
    ActivityLogItem, AnalisisData, ChildRiwayat, ConnRequest, IzinData, KelasData, KelasDetail,
    LaporanAdminData, LaporanGuruExtra, LaporanOrtuData, LaporanSantriData, MaterialItem,
    OutsideRow, PamongData, ParentHome, ParentPermitItem, PermitQueueData, PoinData, ProfilData,
    RiwayatData, SantriHome, SessionUser, SessionsData, StafHome, StudentSearchItem, StudentsData,
    UserControlData,
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
        // Secure di produksi: tanpa itu cookie sesi ikut terkirim lewat HTTP
        // polos dan bisa disadap di jaringan. Di dev (http://localhost) flag
        // ini justru membuat cookie ditolak browser, jadi dikaitkan ke LEPTOS_ENV.
        let secure = if std::env::var("LEPTOS_ENV").as_deref() == Ok("PROD") {
            "; Secure"
        } else {
            ""
        };
        let v = format!(
            "{}={token}; Path=/; HttpOnly; SameSite=Lax{secure}; Max-Age={}",
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
            // "session_expired" utk token yang habis umurnya, "unauth" utk yang
            // cacat. Klien membedakannya agar pesannya jujur: "sesi berakhir,
            // silakan masuk lagi" ≠ "tak berwenang".
            .map_err(|e| ServerFnError::new(e.as_str()))?;
        Ok(claims.into())
    }

    /// Sesi wajib + peran harus salah satu dari `roles`.
    pub async fn require_roles(roles: &[&str]) -> Result<SessionUser, ServerFnError> {
        let s = require_session().await?;
        // ketua=admin, santri_finance=santri (lihat models::role_satisfies).
        if crate::models::role_satisfies(&s.role, roles) {
            Ok(s)
        } else {
            Err(ServerFnError::new("forbidden"))
        }
    }

    /// Satu-satunya pesan yang dilihat pengguna untuk galat yang BUKAN salahnya.
    ///
    /// Ditampilkan `FetchError` sebagai baris penjelas di bawah "Gagal memuat
    /// data", jadi ditulis sebagai kalimat utuh, bukan kode galat.
    pub const GENERIC_SERVER_ERROR: &str =
        "Terjadi gangguan di server. Silakan coba lagi beberapa saat.";

    /// Map error service → ServerFnError, sekaligus memutuskan siapa yang boleh
    /// membaca apa.
    ///
    /// Ada tiga golongan galat, dan hanya dua yang pesannya pantas sampai ke
    /// pengguna:
    ///
    ///   • `UserError` (dari `bail_user!`) — validasi/aturan bisnis. Pesannya
    ///     memang ditulis untuk dibaca pengguna → diteruskan apa adanya, tanpa
    ///     alarm. Diambil dari `.0` (bukan `to_string()`) supaya `.context()`
    ///     yang menumpuk di atasnya tidak menutupi pesan aslinya.
    ///   • Penanda sesi (unauth/forbidden/session_expired) — klien BERCABANG
    ///     pada string ini lewat `is_auth_error`, jadi wajib utuh. Tak
    ///     dialarmkan: peristiwa rutin, dan mengalarmkannya memberi penyerang
    ///     cara membanjiri admin.
    ///   • Sisanya = server yang bermasalah. Rantai `anyhow`-nya memuat teks
    ///     galat Postgres mentah, nama tabel/kolom, dan isi tiap `.context()`.
    ///     Itu bahan diagnosis, BUKAN untuk layar pengguna — dulu semuanya
    ///     mengalir ke `ServerFnError` dan dirender 200 karakter pertamanya oleh
    ///     `FetchError`. Sekarang detailnya hanya ke log + Telegram, penggunanya
    ///     dapat [`GENERIC_SERVER_ERROR`].
    ///
    /// `tracing::error!` di bawah bukan pelengkap: `telegram::report_error`
    /// langsung berhenti bila Telegram tak dikonfigurasi (mis. saat
    /// pengembangan), jadi tanpa log itu galat yang disembunyikan dari layar
    /// akan hilang sama sekali dan tak bisa ditelusuri.
    pub fn err<E: Into<anyhow::Error>>(e: E) -> ServerFnError {
        let e: anyhow::Error = e.into();

        if let Some(u) = e.downcast_ref::<crate::service::UserError>() {
            return ServerFnError::new(u.0.clone());
        }

        let msg = e.to_string();
        if crate::web::components::is_auth_error(&msg) {
            return ServerFnError::new(msg);
        }

        // `{:#}` merangkai SELURUH sebab (“gagal ambil kelas: relation ... does
        // not exist”), bukan cuma lapisan teratas seperti `to_string()`.
        let detail = format!("{e:#}");
        tracing::error!(error = ?e, "server fn gagal");
        crate::service::telegram::report_error(500, "ServerFn", detail);
        ServerFnError::new(GENERIC_SERVER_ERROR)
    }
}

#[cfg(feature = "ssr")]
use ssr_helpers::*;

// ── Server functions ───────────────────────────────────────────────────────────

/// Login: verifikasi kredensial → set cookie sesi → return path redirect per peran.
#[server(LoginAction, "/api-fn")]
pub async fn login_action(login: String, password: String) -> Result<String, ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    let ok = crate::service::auth::login(&state.pool, &mut redis, &state.jwt, &login, &password)
        .await
        .map_err(err)?;
    set_auth_cookie(&ok.token);
    Ok(ok.redirect)
}

/// Lupa password: kirim password baru via WhatsApp ke nomor HP. Anti-enumerasi —
/// SELALU sukses (tak bocorkan apakah nomor terdaftar).
#[server(ForgotPassword, "/api-fn")]
pub async fn forgot_password_action(phone: String) -> Result<(), ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    crate::service::auth::forgot_password(&state.pool, &mut redis, &state.http, &state.waha, &phone)
        .await
        .map_err(err)
}

/// Ganti kata sandi (user yang sedang login): verifikasi sandi lama → set baru.
#[server(ChangePassword, "/api-fn")]
pub async fn change_password_action(
    old_password: String,
    new_password: String,
) -> Result<(), ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::auth::change_password(&state.pool, sess.id, &old_password, &new_password)
        .await
        .map_err(err)
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

// ── Registrasi via link undangan + OTP WhatsApp ───────────────────────────────

/// Buat link undangan (admin/pamong/dewan guru) — return token mentah;
/// klien merangkai URL `{origin}/register?key={token}`.
#[server(CreateInviteAction, "/api-fn")]
pub async fn create_invite_action(
    role: String,
    max_uses: i64,
    ttl_days: i64,
) -> Result<String, ServerFnError> {
    // Peran STAF (dewan_guru/supervisor) khusus admin — kebijakannya ditegakkan
    // di service::registration::can_invite, bukan di sini, supaya pemanggil baru
    // tak bisa melewatinya.
    let sess = require_roles(&["admin", "supervisor", "dewan_guru"]).await?;
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    crate::service::registration::create_invite(&mut redis, &sess.role, &role, max_uses, ttl_days)
        .await
        .map_err(err)
}

/// Cek link masih hidup (pre-auth) — balas label peran siap-tampil, bukan
/// nilai peran mentah.
#[server(ValidateInviteAction, "/api-fn")]
pub async fn validate_invite_action(
    token: String,
) -> Result<crate::models::InviteInfo, ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    let role = crate::service::registration::invite_role(&mut redis, &token)
        .await
        .map_err(err)?
        .ok_or_else(|| ServerFnError::new("Link registrasi tidak valid atau sudah kedaluwarsa."))?;
    // Peran MENTAH sengaja tak dikirim ke klien — cukup label siap-tampil plus
    // penanda apakah form perlu meminta data mahasiswa (migrasi 47).
    Ok(crate::models::InviteInfo {
        role_label: crate::service::registration::describe_role(&role),
        needs_student_profile: crate::models::needs_student_profile(&role),
    })
}

/// Ajukan registrasi (pre-auth) — generate password+OTP, kirim WA.
#[server(RegisterAction, "/api-fn")]
#[allow(clippy::too_many_arguments)]
pub async fn register_action(
    token: String,
    name: String,
    phone: String,
    gender: String,
    campus: String,
    major: String,
    entry_year: String,
) -> Result<(), ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    // Diabaikan bila peran undangan bukan santri (divalidasi di service).
    let profile = crate::service::registration::StudentProfile {
        gender, campus, major, entry_year,
    };
    crate::service::registration::initiate_register(
        &state.pool, &mut redis, &state.http, &state.waha, &token, &name, &phone, &profile,
    )
    .await
    .map_err(err)
}

/// Kirim ulang OTP (pre-auth).
#[server(ResendOtpAction, "/api-fn")]
#[allow(clippy::too_many_arguments)]
pub async fn resend_otp_action(
    token: String,
    name: String,
    phone: String,
    gender: String,
    campus: String,
    major: String,
    entry_year: String,
) -> Result<(), ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    let profile = crate::service::registration::StudentProfile {
        gender, campus, major, entry_year,
    };
    crate::service::registration::resend_otp(
        &state.pool, &mut redis, &state.http, &state.waha, &token, &name, &phone, &profile,
    )
    .await
    .map_err(err)
}

/// Cocokkan OTP → buat akun → set cookie sesi → return path redirect.
#[server(VerifyRegisterAction, "/api-fn")]
pub async fn verify_register_action(
    token: String,
    phone: String,
    otp: String,
) -> Result<String, ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    let ok = crate::service::registration::verify_register(
        &state.pool, &mut redis, &state.jwt, &token, &phone, &otp,
    )
    .await
    .map_err(err)?;
    set_auth_cookie(&ok.token);
    Ok(ok.redirect)
}

/// Data dashboard santri (butuh sesi santri; admin boleh utk inspeksi).
#[server(GetSantriHome, "/api-fn")]
pub async fn santri_home() -> Result<SantriHome, ServerFnError> {
    let sess = require_roles(&["santri", "santri_finance", "admin"]).await?;
    let state = app_state().await?;
    crate::service::dashboard::santri_home(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Riwayat kehadiran lengkap santri.
#[server(GetRiwayat, "/api-fn")]
pub async fn riwayat_data() -> Result<RiwayatData, ServerFnError> {
    let sess = require_roles(&["santri", "santri_finance", "admin"]).await?;
    let state = app_state().await?;
    crate::service::santri::riwayat(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Data halaman Ajukan Perizinan.
#[server(GetIzinData, "/api-fn")]
pub async fn izin_data() -> Result<IzinData, ServerFnError> {
    let sess = require_roles(&["santri", "santri_finance", "admin"]).await?;
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
    let sess = require_roles(&["santri", "santri_finance", "admin"]).await?;
    let state = app_state().await?;
    let splits = crate::service::santri::submit_permit(
        &state.pool, sess.id, sess.id, &kind, &start, &end, &reason,
    )
    .await
    .map_err(err)?;
    // Migrasi 46: satu ajuan bisa pecah ke beberapa wali kelas — beri tahu
    // penyetuju TIAP baris (pamong bila 2 langkah + wali kelas bersangkutan).
    crate::service::permits::notify_permit_splits(
        &state.http, &state.waha, &state.pool, sess.id, &splits, false,
    )
    .await;
    Ok(())
}

/// Antrean izin sisi staf — /izin-staf. Antrean menyesuaikan peran (pamong →
/// tahap 1; wali kelas → keputusan final). Dewan guru TIDAK terlibat izin santri
/// (cukup pamong bila 2 tahap + wali kelas); admin tetap sbg superuser.
#[server(GetPermitQueue, "/api-fn")]
pub async fn permit_queue_data() -> Result<PermitQueueData, ServerFnError> {
    let sess = require_roles(&["supervisor", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::permits::permit_queue(&state.pool, &sess.role, sess.id)
        .await
        .map_err(err)
}

/// Setujui/tolak izin (tahap sesuai peran). Dewan guru tak terlibat izin santri.
#[server(DecidePermitAction, "/api-fn")]
pub async fn decide_permit_action(permit_id: i64, approve: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["supervisor", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::permits::decide_permit(&state.pool, permit_id, approve, &sess.role, sess.id)
        .await
        .map_err(err)
}

/// Mode persetujuan izin saat ini (admin) — "two_stage" | "direct_guru".
#[server(GetPermitMode, "/api-fn")]
pub async fn permit_mode_get() -> Result<String, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    Ok(crate::service::permits::approval_mode(&state.pool).await)
}

/// Ubah mode persetujuan izin (admin).
#[server(SetPermitMode, "/api-fn")]
pub async fn permit_mode_set(mode: String) -> Result<(), ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::permits::set_approval_mode(&state.pool, &mode)
        .await
        .map_err(err)
}

/// Reset saldo poin semua santri ke 300 (awal semester, admin). Return jumlah
/// santri ter-reset.
#[server(ResetSemesterPoints, "/api-fn")]
pub async fn reset_semester_points_action() -> Result<i64, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::reset_semester_points(&state.pool)
        .await
        .map_err(err)
}

/// Rekap kehadiran mingguan per-santri (staf) — /rekap-mingguan.
#[server(GetWeeklyRecap, "/api-fn")]
pub async fn weekly_recap_data(
    offset: i32,
) -> Result<crate::models::WeeklyRecapData, ServerFnError> {
    require_roles(&["supervisor", "teacher", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::rekap::weekly_recap(&state.pool, offset)
        .await
        .map_err(err)
}

/// Kreditkan reward mingguan (admin) untuk pekan `offset`. Return (jumlah santri,
/// total poin) dikreditkan.
#[server(CreditWeeklyRewards, "/api-fn")]
pub async fn credit_weekly_rewards_action(offset: i32) -> Result<(i64, i64), ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::rekap::credit_weekly_rewards(&state.pool, offset)
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

/// Ubah profil mahasiswa milik sendiri (kampus/jurusan/gender/tahun masuk).
#[server(UpdateProfileExtra, "/api-fn")]
pub async fn update_profile_action(
    campus: String,
    major: String,
    gender: String,
    entry_year: String,
) -> Result<(), ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::santri::update_profile_extra(
        &state.pool,
        sess.id,
        &campus,
        &major,
        &gender,
        &entry_year,
    )
    .await
    .map_err(err)
}

/// Ubah kontak (email + alamat) milik sendiri — semua peran.
#[server(UpdateContact, "/api-fn")]
pub async fn update_contact_action(email: String, address: String) -> Result<(), ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::santri::update_contact(&state.pool, sess.id, &email, &address)
        .await
        .map_err(err)
}

/// Tambah entri riwayat IPK milik sendiri.
#[server(AddIpk, "/api-fn")]
pub async fn add_ipk_action(semester: String, ipk: String) -> Result<(), ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::santri::add_ipk(&state.pool, sess.id, &semester, &ipk)
        .await
        .map(|_| ())
        .map_err(err)
}

/// Hapus entri riwayat IPK milik sendiri.
#[server(DeleteIpk, "/api-fn")]
pub async fn delete_ipk_action(id: i64) -> Result<(), ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::santri::delete_ipk(&state.pool, sess.id, id)
        .await
        .map_err(err)
}

/// Kalender akademik satu bulan (semua peran login). Staf → semua kelas;
/// santri → kelasnya; ortu → kelas anak terhubung. Scope ditentukan di service
/// dari peran sesi (bukan parameter klien).
#[server(GetAcademicCalendar, "/api-fn")]
pub async fn academic_calendar_data(
    year: i32,
    month: u32,
) -> Result<crate::models::CalendarData, ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::calendar::calendar_data(&state.pool, &sess, year, month)
        .await
        .map_err(err)
}

// ── Semester akademik (migrasi 40) — admin/dewan guru ─────────────────────────

/// Daftar semester akademik yang didefinisikan (admin/dewan guru).
#[server(GetSemesters, "/api-fn")]
pub async fn semesters_data() -> Result<Vec<crate::models::SemesterItem>, ServerFnError> {
    require_roles(&["admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::semester::list_semesters(&state.pool).await.map_err(err)
}

/// Buat semester baru (ganjil/genap, tahun, rentang tanggal). Return id.
#[server(CreateSemester, "/api-fn")]
pub async fn create_semester_action(
    kind: String,
    year: String,
    start_date: String,
    end_date: String,
) -> Result<i64, ServerFnError> {
    require_roles(&["admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::semester::create_semester(&state.pool, &kind, &year, &start_date, &end_date)
        .await
        .map_err(err)
}

/// Sunting semester yang sudah ada (ganjil/genap, tahun, rentang tanggal).
#[server(UpdateSemester, "/api-fn")]
pub async fn update_semester_action(
    id: i64,
    kind: String,
    year: String,
    start_date: String,
    end_date: String,
) -> Result<(), ServerFnError> {
    require_roles(&["admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::semester::update_semester(&state.pool, id, &kind, &year, &start_date, &end_date)
        .await
        .map_err(err)
}

/// Hapus sebuah semester.
#[server(DeleteSemester, "/api-fn")]
pub async fn delete_semester_action(id: i64) -> Result<(), ServerFnError> {
    require_roles(&["admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::semester::delete_semester(&state.pool, id).await.map_err(err)
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
        .map_err(err)?;
    state.notify_live(session_id); // dorong SSE → pendengar refetch chat
    Ok(())
}

/// Mulai/akhiri sesi live (staf).
#[server(SetSessionLive, "/api-fn")]
pub async fn set_session_live(session_id: i64, start: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::set_live(&state.pool, &sess, session_id, start)
        .await
        .map_err(err)?;
    state.notify_live(session_id); // dorong SSE → pendengar refetch status
    if !start {
        // Sesi diakhiri → pindahkan rekaman lokal ke RustFS (background).
        crate::service::recording::finalize_async(state, session_id);
    }
    Ok(())
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

/// Tandai BANYAK santri (hadir/alpa) sekaligus pada sesi (staf). `status` =
/// "present" | "absent". Return jumlah baru tercatat.
#[server(MarkSessionBulk, "/api-fn")]
pub async fn mark_session_bulk(
    session_id: i64,
    student_ids: Vec<i64>,
    status: String,
) -> Result<i64, ServerFnError> {
    let sess = require_roles(&["teacher", "supervisor", "admin", "dewan_guru"]).await?;
    let state = app_state().await?;
    crate::service::sessions::mark_bulk(&state.pool, &sess, session_id, student_ids, &status)
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
    let splits = crate::service::parent::submit_child_permit(
        &state.pool, sess.id, child_id, &kind, &start, &end, &reason,
    )
    .await
    .map_err(err)?;
    // Migrasi 46: satu ajuan bisa pecah ke beberapa wali kelas — pemohon = ortu.
    crate::service::permits::notify_permit_splits(
        &state.http, &state.waha, &state.pool, child_id, &splits, true,
    )
    .await;
    Ok(())
}

// Migrasi 46: server fn `confirm_child_permit_action` DIHAPUS — orang tua
// tak lagi menyetujui izin (kini pamong kelas → wali kelas per kelas dilewati).

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
    let sess = require_roles(&["supervisor", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    // Pamong (supervisor) → hanya kelas yang ia ampu; admin/dewan → semua.
    let pamong_id = (sess.role == "supervisor").then_some(sess.id);
    crate::service::attendance::pamong_data(&state.pool, pamong_id)
        .await
        .map_err(err)
}

/// Setujui/tolak satu absensi (tahap pamong).
#[server(DecidePamong, "/api-fn")]
pub async fn decide_pamong(att_id: i64, approve: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["supervisor", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    let pamong_id = (sess.role == "supervisor").then_some(sess.id);
    crate::service::attendance::decide_pamong(&state.pool, att_id, sess.id, approve, pamong_id)
        .await
        .map_err(err)?;
    Ok(())
}

/// Antrean verifikasi FINAL (ustad bertugas / dewan guru / admin).
#[server(GetVerifyData, "/api-fn")]
pub async fn verify_data() -> Result<PamongData, ServerFnError> {
    let sess = require_roles(&["teacher", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    // Ustad (teacher) → hanya sesi yang ia ampu; dewan guru/admin → semua.
    let teacher_id = (sess.role == "teacher").then_some(sess.id);
    crate::service::attendance::verify_data(&state.pool, teacher_id)
        .await
        .map_err(err)
}

/// Verifikasi final satu absensi (oleh ustad bertugas / dewan guru / admin).
#[server(DecideVerify, "/api-fn")]
pub async fn decide_verify(att_id: i64, approve: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["teacher", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    let teacher_id = (sess.role == "teacher").then_some(sess.id);
    crate::service::attendance::decide_verify(&state.pool, att_id, sess.id, approve, teacher_id)
        .await
        .map_err(err)?;
    Ok(())
}

/// Panel verifikasi kehadiran PER-SESI (detail sesi). Tahap otomatis dari peran:
/// pamong → tahap pamong; dewan guru/admin → final. Kosong = tak ada yg menunggu.
#[server(GetSessionVerify, "/api-fn")]
pub async fn session_verify_data(
    session_id: i64,
) -> Result<crate::models::SessionVerifyData, ServerFnError> {
    let sess = require_roles(&["supervisor", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::attendance::session_verify(&state.pool, session_id, &sess.role, sess.id)
        .await
        .map_err(err)
}

/// Verifikasi SATU sesi sekaligus: setujui semua yang menunggu KECUALI `reject_ids`
/// (yang ditolak). Satu request per sesi (bukan per santri). Return jumlah diproses.
#[server(DecideSession, "/api-fn")]
pub async fn decide_session_action(
    session_id: i64,
    reject_ids: Vec<i64>,
) -> Result<i64, ServerFnError> {
    let sess = require_roles(&["supervisor", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::attendance::decide_session(&state.pool, session_id, &sess.role, sess.id, &reject_ids)
        .await
        .map_err(err)
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

/// Buat kelas baru (nama + kategori + golongan, semua fleksibel).
#[server(CreateClass, "/api-fn")]
pub async fn create_class_action(
    name: String,
    category: String,
    golongan: String,
    description: String,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::create_class(&state.pool, &name, &category, &golongan, &description)
        .await
        .map_err(err)
}

/// Ubah kelas (nama + kategori + golongan) — Edit Detail Kelas.
#[server(UpdateClass, "/api-fn")]
pub async fn update_class_action(
    class_id: i64,
    name: String,
    category: String,
    golongan: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::update_class(&state.pool, class_id, &name, &category, &golongan)
        .await
        .map_err(err)
}

/// Tetapkan wali kelas + pamong + rute persetujuan izin (perlu pamong?) satu kelas.
#[server(SetClassStaff, "/api-fn")]
pub async fn set_class_staff_action(
    class_id: i64,
    wali_kelas_id: i64,
    pamong_id: i64,
    require_pamong: bool,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_class_staff(
        &state.pool,
        class_id,
        wali_kelas_id,
        pamong_id,
        require_pamong,
    )
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
    category: String,
    present_points: String,
    late_points: String,
    absent_points: String,
    room_id: i64,
    custom_dates: String,
    activity_type: String,
    izin_points: String,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::create_schedule(
        &state.pool, class_id, &title, &start_time, &end_time, &limit_time, &recurrence,
        &start_date, &end_date, &category, &present_points, &late_points, &absent_points, room_id,
        &custom_dates, &activity_type, &izin_points,
    )
    .await
    .map_err(err)
}

/// Buat sesi baru untuk sebuah kelas. `book_id` 0 = tanpa materi buku.
#[server(CreateSession, "/api-fn")]
pub async fn create_session_action(
    class_id: i64,
    schedule_id: i64,
    teacher_id: i64,
    title: String,
    session_date: String,
    book_id: i64,
    book_pages: String,
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
        Some(book_id),
        &book_pages,
    )
    .await
    .map_err(err)
}

/// Ubah materi buku sesi yang sudah ada (tab "Kelola" /sesi/:id).
#[server(SetSessionBook, "/api-fn")]
pub async fn set_session_book_action(
    session_id: i64,
    book_id: i64,
    book_pages: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_session_book(&state.pool, session_id, book_id, &book_pages)
        .await
        .map_err(err)
}

/// Set materi TARGET/rencana sesi (migrasi 41). book_id 0 = kosongkan.
#[server(SetSessionTarget, "/api-fn")]
pub async fn set_session_target_action(
    session_id: i64,
    book_id: i64,
    book_pages: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_session_target(&state.pool, session_id, book_id, &book_pages)
        .await
        .map_err(err)
}

/// Set catatan ayat/hadith AKTUAL sesi (teks bebas, migrasi 41).
#[server(SetSessionActualDetail, "/api-fn")]
pub async fn set_session_actual_detail_action(
    session_id: i64,
    detail: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_session_actual_detail(&state.pool, session_id, &detail)
        .await
        .map_err(err)
}

/// Tambah santri ke KELAS — berlaku untuk semua jadwal kelas itu (migrasi 61).
#[server(AddMember, "/api-fn")]
pub async fn add_member_action(class_id: i64, student_id: i64) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::add_member(&state.pool, class_id, student_id).await.map_err(err)
}

/// Tambah BANYAK santri ke KELAS sekali request. Return jumlah baru ditambahkan.
#[server(AddMembers, "/api-fn")]
pub async fn add_members_action(
    class_id: i64,
    student_ids: Vec<i64>,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::add_members(&state.pool, class_id, student_ids).await.map_err(err)
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
    category: String,
    present_points: String,
    late_points: String,
    absent_points: String,
    room_id: i64,
    custom_dates: String,
    activity_type: String,
    izin_points: String,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::update_schedule(
        &state.pool, schedule_id, &title, &start_time, &end_time, &limit_time, &recurrence,
        &start_date, &end_date, &category, &present_points, &late_points, &absent_points, room_id,
        &custom_dates, &activity_type, &izin_points,
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

/// Pasang/ubah PAMONG bertugas verifikasi sebuah sesi (0 = kosongkan) — migrasi 33.
#[server(SetSessionPamong, "/api-fn")]
pub async fn set_session_pamong_action(session_id: i64, pamong_id: i64) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_session_pamong(&state.pool, session_id, pamong_id)
        .await
        .map_err(err)
}

/// Kirim jadwal sesi ke Google Calendar santri via WhatsApp (tautan "Tambah ke
/// Google Calendar"). Staf. Return (terkirim, dilewati_tanpa_HP).
#[server(SendScheduleWa, "/api-fn")]
pub async fn send_schedule_wa_action(session_id: i64) -> Result<(i64, i64), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::calendar::send_schedule_wa(&state.pool, &state.http, &state.waha, session_id)
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
pub async fn staff_search_students(
    q: String,
    class_id: i64,
) -> Result<Vec<StudentSearchItem>, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::search_students(&state.pool, &q, class_id)
        .await
        .map_err(err)
}

/// Tambah materi/kitab ke kurikulum kelas (migrasi 17).
#[server(CreateCurriculum, "/api-fn")]
pub async fn create_curriculum_action(
    class_id: i64,
    book_id: i64,
    start_surah: i32,
    start_unit: i32,
    end_surah: i32,
    end_unit: i32,
    cur_surah: i32,
    cur_unit: i32,
) -> Result<i64, ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::create_curriculum(
        &state.pool, class_id, book_id, start_surah, start_unit, end_surah, end_unit,
        cur_surah, cur_unit,
    )
    .await
    .map_err(err)
}

/// Ubah materi/kitab kurikulum.
#[server(UpdateCurriculum, "/api-fn")]
pub async fn update_curriculum_action(
    id: i64,
    book_id: i64,
    start_surah: i32,
    start_unit: i32,
    end_surah: i32,
    end_unit: i32,
    cur_surah: i32,
    cur_unit: i32,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::update_curriculum(
        &state.pool, id, book_id, start_surah, start_unit, end_surah, end_unit, cur_surah,
        cur_unit,
    )
    .await
    .map_err(err)
}

/// Kelas-kelas yang diikuti santri yang sedang masuk: kurikulum, materi yang
/// sedang dibahas, teman sekelas, wali kelas & pamong.
#[server(SantriKelas, "/api-fn")]
pub async fn santri_kelas_data() -> Result<crate::models::SantriKelasData, ServerFnError> {
    // Santri hanya boleh melihat kelasnya SENDIRI — id diambil dari sesi,
    // bukan dari parameter, supaya tak bisa mengintip kelas orang lain.
    let sess = require_roles(&["santri", "santri_finance"]).await?;
    let state = app_state().await?;
    crate::service::kelas::santri_kelas(&state.pool, sess.id).await.map_err(err)
}

/// Setel materi & posisi yang SEDANG BERJALAN pada satu jadwal (migrasi 57).
#[server(SetScheduleCurrent, "/api-fn")]
pub async fn set_schedule_current_action(
    schedule_id: i64,
    book_id: i64,
    surah: i32,
    unit: i32,
) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::set_schedule_current(&state.pool, schedule_id, book_id, surah, unit)
        .await
        .map_err(err)
}

/// Hapus materi/kitab kurikulum.
#[server(DeleteCurriculum, "/api-fn")]
pub async fn delete_curriculum_action(id: i64) -> Result<(), ServerFnError> {
    require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::delete_curriculum(&state.pool, id).await.map_err(err)
}

// ── Materials Library (migrasi 17; admin/dewan_guru) ──────────────────────────

#[cfg(feature = "ssr")]
const MATERIALS_ROLES: &[&str] = &["admin", "dewan_guru"];

/// Daftar materi terbaru (widget dashboard + halaman /materi).
#[server(GetMaterials, "/api-fn")]
pub async fn materials_list(limit: i64) -> Result<Vec<MaterialItem>, ServerFnError> {
    require_roles(MATERIALS_ROLES).await?;
    let state = app_state().await?;
    crate::service::materials::list_materials(&state.pool, limit)
        .await
        .map_err(err)
}

/// Tambah materi berupa TAUTAN (mis. video YouTube) — file lewat
/// POST /api/materials/upload (multipart, di luar server-fn).
#[server(AddMaterialLink, "/api-fn")]
pub async fn add_material_link_action(title: String, url: String) -> Result<i64, ServerFnError> {
    let sess = require_roles(MATERIALS_ROLES).await?;
    let state = app_state().await?;
    crate::service::materials::add_link(&state.pool, sess.id, &title, &url)
        .await
        .map_err(err)
}

/// Hapus materi.
#[server(DeleteMaterialAction, "/api-fn")]
pub async fn delete_material_action(id: i64) -> Result<(), ServerFnError> {
    require_roles(MATERIALS_ROLES).await?;
    let state = app_state().await?;
    crate::service::materials::delete_material(&state.pool, id).await.map_err(err)
}

// ── Galeri Foto Kegiatan (migrasi 34) ─────────────────────────────────────────
// Tampil publik di beranda; kelola (hapus/urutkan) hanya admin/dewan_guru.
// Upload file lewat POST /api/activity-photos/upload (multipart, di luar sini).

#[cfg(feature = "ssr")]
const GALLERY_MANAGE_ROLES: &[&str] = &["admin", "dewan_guru"];

/// Daftar foto kegiatan terurut — PUBLIK (dipakai beranda + halaman kelola).
#[server(GetActivityPhotos, "/api-fn")]
pub async fn activity_photos_data() -> Result<Vec<crate::models::ActivityPhoto>, ServerFnError> {
    let state = app_state().await?;
    crate::repository::list_activity_photos(&state.pool).await.map_err(err)
}

/// Hapus satu foto kegiatan (admin/dewan_guru).
#[server(DeleteActivityPhoto, "/api-fn")]
pub async fn delete_activity_photo_action(id: i64) -> Result<(), ServerFnError> {
    require_roles(GALLERY_MANAGE_ROLES).await?;
    let state = app_state().await?;
    crate::repository::delete_activity_photo(&state.pool, id).await.map(|_| ()).map_err(err)
}

/// Simpan urutan baru hasil drag-and-drop (admin/dewan_guru). `ids` = urutan
/// tampil dari kiri-atas ke kanan-bawah.
#[server(ReorderActivityPhotos, "/api-fn")]
pub async fn reorder_activity_photos_action(ids: Vec<i64>) -> Result<(), ServerFnError> {
    require_roles(GALLERY_MANAGE_ROLES).await?;
    let state = app_state().await?;
    crate::repository::reorder_activity_photos(&state.pool, &ids).await.map_err(err)
}

/// Simpan titik fokus & perbesaran satu foto (admin/dewan_guru, migrasi 54).
///
/// Nilai dirapikan di SERVER lewat `clamp_focus`, bukan hanya di editor: sebuah
/// server function bisa dipanggil langsung, dan nilai di luar rentang akan
/// menabrak CHECK di tabel sehingga pengguna hanya melihat galat Postgres.
#[server(SetActivityPhotoFocus, "/api-fn")]
pub async fn set_activity_photo_focus_action(
    id: i64,
    focus_x: f32,
    focus_y: f32,
    zoom: f32,
    fit: String,
) -> Result<(), ServerFnError> {
    require_roles(GALLERY_MANAGE_ROLES).await?;
    let state = app_state().await?;
    let (fx, fy, z) = crate::models::clamp_focus(focus_x, focus_y, zoom);
    let framing = crate::repository::PhotoFraming {
        focus_x: fx,
        focus_y: fy,
        zoom: z,
        fit: crate::models::PhotoFit::from_str(&fit).as_str(),
    };
    crate::repository::set_activity_photo_focus(&state.pool, id, framing)
        .await
        .map(|_| ())
        .map_err(err)
}

// ── Buku tamu (migrasi 35) — PUBLIK (tanpa login) ─────────────────────────────
// /tamu: isi data → kode 6-digit (Redis) → ketik di mesin IoT → mesin kirim kode
// + wajah ke POST /api/guestbook → HP tamu polling status → ✅.

/// Daftar tamu → balas kode spesial 6-digit untuk diketik di mesin.
#[server(RegisterGuest, "/api-fn")]
pub async fn register_guest_action(
    name: String,
    phone: String,
    purpose: String,
) -> Result<String, ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    crate::service::guest::register_guest(&mut redis, &name, &phone, &purpose)
        .await
        .map_err(err)
}

/// Polling status check-in tamu. Some = mesin sudah konfirmasi (tampil ✅ + wajah).
#[server(GuestStatus, "/api-fn")]
pub async fn guest_status_action(
    code: String,
) -> Result<Option<crate::models::GuestCheckin>, ServerFnError> {
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    crate::service::guest::check_status(&mut redis, &code)
        .await
        .map_err(err)
}

// ── Tagihan santri (migrasi 37) ───────────────────────────────────────────────
// FINANCE = lihat belum-bayar + tandai lunas + verifikasi (admin, ketua,
// santri_finance). BILL_ADMIN = buat/hapus tagihan (admin, ketua).

#[cfg(feature = "ssr")]
const FINANCE_ROLES: &[&str] = &["admin", "ketua", "santri_finance"];
#[cfg(feature = "ssr")]
const BILL_ADMIN_ROLES: &[&str] = &["admin", "ketua"];

/// Daftar santri yang BELUM membayar (finance).
#[server(GetUnpaidBills, "/api-fn")]
pub async fn unpaid_bills_data() -> Result<Vec<crate::models::BillItem>, ServerFnError> {
    require_roles(FINANCE_ROLES).await?;
    let state = app_state().await?;
    crate::repository::list_unpaid(&state.pool, 500).await.map_err(err)
}

/// Riwayat pembayaran santri yang SUDAH lunas (finance) — periode, nominal,
/// metode, bukti TF, terbaru dibayar dulu.
#[server(GetPaidBills, "/api-fn")]
pub async fn paid_bills_data() -> Result<Vec<crate::models::BillItem>, ServerFnError> {
    require_roles(FINANCE_ROLES).await?;
    let state = app_state().await?;
    crate::repository::list_paid(&state.pool, 500).await.map_err(err)
}

/// Buat tagihan untuk seorang santri (admin/ketua). Tanggal "YYYY-MM-DD".
#[server(CreateBill, "/api-fn")]
pub async fn create_bill_action(
    user_id: i64,
    title: String,
    price: i64,
    started: String,
    expired: String,
    note: String,
) -> Result<i64, ServerFnError> {
    require_roles(BILL_ADMIN_ROLES).await?;
    let state = app_state().await?;
    let parse = |s: &str| chrono::NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d");
    let sd = parse(&started).map_err(|_| ServerFnError::new("Tanggal mulai tidak valid (YYYY-MM-DD)."))?;
    let ed = parse(&expired).map_err(|_| ServerFnError::new("Tanggal jatuh tempo tidak valid."))?;
    if price <= 0 {
        return Err(ServerFnError::new("Nominal tagihan harus > 0."));
    }
    crate::repository::create_bill(&state.pool, user_id, title.trim(), price, sd, ed, note.trim())
        .await
        .map_err(err)
}

/// Tandai tagihan LUNAS + verifikasi (finance). `paid_amount` None = penuh.
#[server(MarkBillPaid, "/api-fn")]
pub async fn mark_bill_paid_action(
    bill_id: i64,
    paid_amount: Option<i64>,
    method: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(FINANCE_ROLES).await?;
    // Pemegang peran santri_finance TIDAK boleh menyetujui tagihannya sendiri —
    // memverifikasi pembayaran diri sendiri meniadakan gunanya verifikasi.
    // Admin/ketua tak dibatasi (mereka bukan pihak yang ditagih).
    if crate::models::role_satisfies(&sess.role, &["santri"]) {
        let state = app_state().await?;
        if crate::repository::bill_owner(&state.pool, bill_id).await.map_err(err)? == Some(sess.id) {
            return Err(ServerFnError::new(
                "Tagihan Anda sendiri harus diverifikasi pengurus lain.",
            ));
        }
    }
    let state = app_state().await?;
    crate::repository::mark_paid(&state.pool, bill_id, paid_amount, method.trim(), sess.id)
        .await
        .map(|_| ())
        .map_err(err)
}

/// Hapus tagihan (admin/ketua).
#[server(DeleteBill, "/api-fn")]
pub async fn delete_bill_action(bill_id: i64) -> Result<(), ServerFnError> {
    require_roles(BILL_ADMIN_ROLES).await?;
    let state = app_state().await?;
    crate::repository::delete_bill(&state.pool, bill_id).await.map(|_| ()).map_err(err)
}

/// Tagihan MILIK santri yang login (dashboard santri).
#[server(GetMyBills, "/api-fn")]
pub async fn my_bills_data() -> Result<Vec<crate::models::BillItem>, ServerFnError> {
    let sess = require_roles(&["santri", "santri_finance"]).await?;
    let state = app_state().await?;
    crate::repository::list_for_user(&state.pool, sess.id).await.map_err(err)
}

/// Cari santri untuk membuat tagihan (admin/ketua) — nama/NIS.
#[server(FinanceSearchStudents, "/api-fn")]
pub async fn finance_student_search(q: String) -> Result<Vec<StudentSearchItem>, ServerFnError> {
    require_roles(BILL_ADMIN_ROLES).await?;
    let state = app_state().await?;
    crate::service::kelas::search_students(&state.pool, &q, 0)
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

// ── Buku materi hafalan + progres santri (migrasi 18) ─────────────────────────
// Lihat/pilih buku: semua peran staf. Kelola buku + isi progres: admin/pamong/
// guru/dewan guru (`BOOKS_MANAGE_ROLES`) — sama dgn admin, sesuai permintaan user.

#[cfg(feature = "ssr")]
const BOOKS_VIEW_ROLES: &[&str] = &["admin", "dewan_guru", "supervisor", "teacher"];
#[cfg(feature = "ssr")]
const BOOKS_MANAGE_ROLES: &[&str] = &["admin", "supervisor", "teacher", "dewan_guru"];

/// Daftar buku aktif (dropdown/panel progres).
#[server(GetBooks, "/api-fn")]
pub async fn books_list() -> Result<Vec<crate::models::BookItem>, ServerFnError> {
    require_roles(BOOKS_VIEW_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::list_books(&state.pool).await.map_err(err)
}

/// Tambah materi baru. `category` = "quran" | "hadist". Hadist → total_pages;
/// quran → surahs (JSON `[{"name","ayat"}]`).
#[server(CreateBookAction, "/api-fn")]
pub async fn create_book_action(
    title: String,
    category: String,
    total_pages: String,
    surahs: String,
) -> Result<i64, ServerFnError> {
    require_roles(BOOKS_MANAGE_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::create_book(&state.pool, &title, &category, &total_pages, &surahs)
        .await
        .map_err(err)
}

/// Ubah materi (admin/pamong/guru). Sama parameter dgn create.
#[server(UpdateBookAction, "/api-fn")]
pub async fn update_book_action(
    id: i64,
    title: String,
    category: String,
    total_pages: String,
    surahs: String,
) -> Result<(), ServerFnError> {
    require_roles(BOOKS_MANAGE_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::update_book(&state.pool, id, &title, &category, &total_pages, &surahs)
        .await
        .map_err(err)
}

/// Hapus buku (soft delete, admin/pamong).
#[server(DeleteBookAction, "/api-fn")]
pub async fn delete_book_action(id: i64) -> Result<(), ServerFnError> {
    require_roles(BOOKS_MANAGE_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::delete_book(&state.pool, id).await.map_err(err)
}

/// Audit akademik semua santri (tab "Akademik" /students) — rata-rata
/// persentase lintas buku, paling tertinggal duluan.
#[server(GetAcademicAudit, "/api-fn")]
pub async fn academic_audit_data() -> Result<Vec<crate::models::StudentAcademicItem>, ServerFnError> {
    require_roles(BOOKS_VIEW_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::academic_audit(&state.pool).await.map_err(err)
}

/// Progres satu santri di semua buku aktif (panel di /students).
#[server(GetStudentBookProgress, "/api-fn")]
pub async fn student_book_progress_data(
    user_id: i64,
) -> Result<Vec<crate::models::BookProgressItem>, ServerFnError> {
    require_roles(BOOKS_VIEW_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::student_progress(&state.pool, user_id)
        .await
        .map_err(err)
}

/// Progres satu santri dilihat oleh PENGGUNA LAIN (orang tua terhubung /
/// staf / guru / santri sendiri). Authorization di service, bukan hanya role.
#[server(GetStudentBookProgressForViewer, "/api-fn")]
pub async fn student_book_progress_for_viewer(
    user_id: i64,
) -> Result<Vec<crate::models::BookProgressItem>, ServerFnError> {
    let sess = require_session().await?;
    let state = app_state().await?;
    crate::service::books::student_progress_for_viewer(
        &state.pool,
        sess.id,
        &sess.role,
        user_id,
    )
    .await
    .map_err(err)
}

/// Simpan progres per-unit satu santri pada satu materi (peta unit→status JSON,
/// admin/pamong/guru).
#[server(SetStudentBookProgressAction, "/api-fn")]
pub async fn set_student_book_progress_action(
    user_id: i64,
    book_id: i64,
    unit_status: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(BOOKS_MANAGE_ROLES).await?;
    let state = app_state().await?;
    crate::service::books::set_unit_status(&state.pool, sess.id, user_id, book_id, &unit_status)
        .await
        .map_err(err)
}

/// Progres akademik SANTRI SENDIRI (buku yang sudah didaftarkan admin) —
/// halaman /akademik. Reuse `student_progress` (sama dgn panel admin), tapi
/// `user_id` diambil dari sesi (bukan parameter) — santri tak bisa lihat
/// punya orang lain.
#[server(GetOwnBookProgress, "/api-fn")]
pub async fn own_book_progress_data() -> Result<Vec<crate::models::BookProgressItem>, ServerFnError> {
    let sess = require_roles(&["santri", "santri_finance"]).await?;
    let state = app_state().await?;
    crate::service::books::student_progress(&state.pool, sess.id)
        .await
        .map_err(err)
}

/// Santri mengisi SENDIRI progres per-unit materinya (grid penuh/setengah/
/// kosong) — halaman /akademik. `user_id` = sess.id (tak terima dari klien).
#[server(SetOwnBookProgressAction, "/api-fn")]
pub async fn set_own_book_progress_action(
    book_id: i64,
    unit_status: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(&["santri", "santri_finance"]).await?;
    let state = app_state().await?;
    crate::service::books::set_unit_status(&state.pool, sess.id, sess.id, book_id, &unit_status)
        .await
        .map_err(err)
}

// ── Sisi STAF / GURU / DEWAN GURU ────────────────────────────────────────────────

/// Dashboard staf (/staf) — statistik hari ini, sesi live, kehadiran terbaru.
#[server(GetStafHome, "/api-fn")]
pub async fn staf_home_data() -> Result<StafHome, ServerFnError> {
    let sess = require_roles(&["admin", "ketua", "dewan_guru", "supervisor"]).await?;
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

// ── Laporan (/laporan — menggantikan item Profil di navbar) ─────────────────────

/// Laporan Institusi — admin & pamong.
#[server(GetLaporanAdmin, "/api-fn")]
pub async fn laporan_admin_data() -> Result<LaporanAdminData, ServerFnError> {
    require_roles(&["admin", "supervisor"]).await?;
    let state = app_state().await?;
    crate::service::laporan::laporan_admin(&state.pool).await.map_err(err)
}

/// Ekstensi laporan guru/dewan guru: ranking hafalan "Santri Teladan" (dipanggil
/// BERSAMA `analisis_data` yang sudah ada — kartu kehadiran/tren/ranking kelas
/// dipakai apa adanya, di sini hanya tambahan hafalan).
#[server(GetLaporanGuruExtra, "/api-fn")]
pub async fn laporan_guru_extra_data() -> Result<LaporanGuruExtra, ServerFnError> {
    require_roles(&["teacher", "dewan_guru", "admin"]).await?;
    let state = app_state().await?;
    crate::service::laporan::laporan_guru_extra(&state.pool).await.map_err(err)
}

/// Rapor Pribadi Santri.
#[server(GetLaporanSantri, "/api-fn")]
pub async fn laporan_santri_data() -> Result<LaporanSantriData, ServerFnError> {
    let sess = require_roles(&["santri"]).await?;
    let state = app_state().await?;
    crate::service::laporan::laporan_santri(&state.pool, &sess).await.map_err(err)
}

/// Laporan Santri untuk Orang Tua (anak terhubung; `child` None = anak pertama).
#[server(GetLaporanOrtu, "/api-fn")]
pub async fn laporan_ortu_data(child: Option<i64>) -> Result<LaporanOrtuData, ServerFnError> {
    let sess = require_roles(&["parent"]).await?;
    let state = app_state().await?;
    crate::service::laporan::laporan_ortu(&state.pool, sess.id, child).await.map_err(err)
}

/// Santri yang sedang berstatus "di luar pondok" (gerbang RFID) — laporan
/// admin/pamong.
#[server(GetOutside, "/api-fn")]
pub async fn students_outside_action() -> Result<Vec<OutsideRow>, ServerFnError> {
    require_roles(&["admin", "supervisor"]).await?;
    let state = app_state().await?;
    let rows = crate::repository::students_outside(&state.pool, 30).await.map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|r| OutsideRow {
            user_id: r.user_id,
            name: r.name,
            nis: r.nis.unwrap_or_else(|| "-".into()),
            class_name: r.class_name.unwrap_or_else(|| "-".into()),
            since_label: r
                .gate_at
                .map(crate::service::fmt::fmt_dt_full)
                .unwrap_or_else(|| "-".into()),
        })
        .collect())
}

/// Catat setoran hafalan santri (staf saja) — panel di detail sesi kategori
/// "Mengaji"/"Pengajian".
#[server(LogHafalan, "/api-fn")]
pub async fn log_hafalan_action(
    student_id: i64,
    class_id: Option<i64>,
    surah: String,
    ayat_range: String,
    juz: Option<i16>,
    quality: String,
    note: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin", "supervisor", "dewan_guru", "teacher"]).await?;
    let state = app_state().await?;
    crate::service::hafalan::log_hafalan(
        &state.pool, &sess, student_id, class_id, &surah, &ayat_range, juz, &quality, &note,
    )
    .await
    .map_err(err)
}

/// Setoran hafalan terbaru satu kelas (panel di detail sesi).
#[server(HafalanOfClass, "/api-fn")]
pub async fn hafalan_of_class_action(
    class_id: i64,
) -> Result<Vec<(String, crate::models::HafalanItem)>, ServerFnError> {
    require_roles(&["admin", "supervisor", "dewan_guru", "teacher"]).await?;
    let state = app_state().await?;
    crate::service::hafalan::hafalan_of_class(&state.pool, class_id, 15).await.map_err(err)
}

// ── User Control (admin-only, migrasi 17) ─────────────────────────────────────
// Nav "User Control" tampil di SEMUA peran staf (uniform), tapi akses data
// tetap admin-only — server fn di sini yang menegakkannya, bukan nav.

/// Daftar user + statistik (`role_filter` kosong = semua peran).
#[server(GetUserControlData, "/api-fn")]
pub async fn user_control_data(role_filter: String) -> Result<UserControlData, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    let filter = (!role_filter.is_empty()).then_some(role_filter);
    crate::service::admin::user_control_data(&state.pool, filter.as_deref())
        .await
        .map_err(err)
}

/// Jejak aksi administratif terbaru (panel Activity Logs).
#[server(GetActivityLog, "/api-fn")]
pub async fn activity_log_data() -> Result<Vec<ActivityLogItem>, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::recent_activity(&state.pool, 20).await.map_err(err)
}

/// Aktifkan/nonaktifkan akun.
#[server(ToggleUserActiveAction, "/api-fn")]
pub async fn toggle_user_active_action(user_id: i64, active: bool) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::toggle_active(&state.pool, sess.id, user_id, active)
        .await
        .map_err(err)
}

/// Ganti peran user.
#[server(ChangeUserRoleAction, "/api-fn")]
pub async fn change_user_role_action(user_id: i64, new_role: String) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::change_role(&state.pool, sess.id, user_id, &new_role)
        .await
        .map_err(err)
}

// ── Perangkat RFID (ruang) — manajemen admin ─────────────────────────────────

// ── Pendaftaran kartu RFID (admin) ────────────────────────────────────────────

/// Kartu tak dikenal yang baru ditempel di mesin (Redis, hidup 1 jam).
/// Admin memasangkannya ke pengguna tanpa mengetik nomor kartu.
#[server(GetPendingCards, "/api-fn")]
pub async fn pending_cards_data() -> Result<Vec<crate::models::PendingCardItem>, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    crate::service::enrollment::pending_cards(&mut redis).await.map_err(err)
}

/// Cari pengguna (SEMUA peran) untuk dipasangi kartu.
#[server(SearchUsersForCard, "/api-fn")]
pub async fn search_users_for_card(
    q: String,
) -> Result<Vec<crate::models::UserPickItem>, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    if q.trim().chars().count() < 2 {
        return Ok(Vec::new());
    }
    let rows = crate::repository::search_users_for_card(&state.pool, &q, 20)
        .await
        .map_err(err)?;
    Ok(rows
        .into_iter()
        .map(|r| crate::models::UserPickItem {
            id: r.id,
            full_name: r.full_name,
            role_label: crate::service::registration::describe_role(&r.role),
            nis: r.nis.unwrap_or_else(|| "-".into()),
            current_card: r.rfid_cards.unwrap_or(0),
        })
        .collect())
}

/// Pasang kartu ke pengguna.
#[server(AssignRfidCard, "/api-fn")]
pub async fn assign_card_action(user_id: i64, card: i64) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin"]).await?;
    let state = app_state().await?;
    let mut redis = state.redis.clone();
    crate::service::enrollment::assign_card(&state.pool, &mut redis, user_id, card, sess.id)
        .await
        .map_err(err)
}

/// Lepas kartu dari pengguna (hilang/rusak).
#[server(UnassignRfidCard, "/api-fn")]
pub async fn unassign_card_action(user_id: i64) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::enrollment::unassign_card(&state.pool, user_id, sess.id)
        .await
        .map_err(err)
}

/// Koreksi status absensi (mis. alpa keliru → hadir).
///
/// Wewenangnya SENGAJA sempit: hanya guru pengisi atau pamong yang bertugas di
/// sesi itu. Ditegakkan di query repository, bukan di sini — supaya pemanggil
/// baru tak bisa melewatinya. `require_roles` di bawah hanya menyaring kasar.
#[server(CorrectAttendance, "/api-fn")]
pub async fn correct_attendance_action(
    att_id: i64,
    new_status: String,
) -> Result<(), ServerFnError> {
    let sess = require_roles(&["admin", "dewan_guru", "supervisor", "teacher"]).await?;
    let state = app_state().await?;
    crate::service::attendance::correct_attendance(&state.pool, att_id, &new_status, sess.id)
        .await
        .map_err(err)
}

/// Daftar perangkat RFID. Admin (User Control) + KELAS_ROLES (dropdown ruang
/// saat buat/ubah jadwal).
///
/// `api_key` HANYA dikirim ke admin. Peran kelas lain (pamong, dewan guru)
/// butuh daftar ini sekadar untuk memilih ruang — mereka tak perlu, dan tak
/// boleh, memegang kunci yang mengizinkan perangkat mencatat absensi. Sebelum
/// ini seluruh kunci ikut terkirim ke semua staf.
#[server(GetRfidDevices, "/api-fn")]
pub async fn rfid_devices_list() -> Result<Vec<crate::models::RfidDeviceItem>, ServerFnError> {
    let sess = require_roles(KELAS_ROLES).await?;
    let state = app_state().await?;
    let mut list = crate::service::admin::rfid_devices(&state.pool).await.map_err(err)?;
    if !crate::models::role_satisfies(&sess.role, &["admin"]) {
        for d in &mut list {
            d.api_key.clear();
        }
    }
    Ok(list)
}

/// Buat perangkat RFID (admin). api_key kosong → di-generate otomatis.
#[server(CreateRfidDevice, "/api-fn")]
pub async fn create_rfid_device_action(
    device_name: String,
    serial_number: String,
    location: String,
    api_key: String,
    category: String,
) -> Result<String, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    // Balas KUNCINYA, bukan id: kunci disimpan sebagai hash (migrasi 53), jadi
    // inilah satu-satunya saat admin bisa melihatnya untuk disalin ke firmware.
    crate::service::admin::create_rfid_device(
        &state.pool, &device_name, &serial_number, &location, &api_key, &category,
    )
    .await
    .map(|(_id, key)| key)
    .map_err(err)
}

/// Ubah perangkat RFID (admin).
#[server(UpdateRfidDevice, "/api-fn")]
pub async fn update_rfid_device_action(
    id: i64,
    device_name: String,
    serial_number: String,
    location: String,
    category: String,
) -> Result<(), ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::update_rfid_device(
        &state.pool, id, &device_name, &serial_number, &location, &category,
    )
    .await
    .map_err(err)
}

/// Ganti api_key perangkat (admin) → return key baru.
#[server(RegenRfidKey, "/api-fn")]
pub async fn regenerate_rfid_key_action(id: i64) -> Result<String, ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::regenerate_rfid_key(&state.pool, id).await.map_err(err)
}

/// Hapus perangkat RFID (admin).
#[server(DeleteRfidDevice, "/api-fn")]
pub async fn delete_rfid_device_action(id: i64) -> Result<(), ServerFnError> {
    require_roles(&["admin"]).await?;
    let state = app_state().await?;
    crate::service::admin::delete_rfid_device(&state.pool, id).await.map_err(err)
}
