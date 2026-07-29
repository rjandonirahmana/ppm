//! service/auth.rs — Login (bcrypt verify → JWT) + bootstrap admin.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use crate::auth::JwtService;
use crate::models::SessionUser;
use crate::repository as repo;

/// Hasil login sukses.
pub struct LoginOk {
    pub user: SessionUser,
    pub token: String,
    /// Path redirect sesuai peran.
    pub redirect: String,
}

/// Normalisasi input jadi bentuk HP tersimpan (08.. → 62..). Non-digit dibuang.
/// Dipakai login (cocokkan phone_number) & forgot-password.
pub fn normalize_phone(s: &str) -> String {
    let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    match d.strip_prefix('0') {
        Some(rest) => format!("62{rest}"),
        None => d,
    }
}

/// Verifikasi kredensial → JWT (pola sama e-ticketing AuthService::login).
/// Login UTAMANYA pakai NOMOR HP; username/email/NIS tetap didukung (admin seed).
/// bcrypt CPU-bound → `spawn_blocking` agar tidak menyumbat worker async.
pub async fn login(pool: &Pool, jwt: &JwtService, login: &str, password: &str) -> Result<LoginOk> {
    let login = login.trim();
    let phone = normalize_phone(login);
    let Some(user) = repo::find_user_for_login(pool, login, &phone).await? else {
        bail!("Nomor HP atau kata sandi salah.");
    };

    let hash = user.password_hash.clone();
    let pw = password.to_string();
    let verify_start = std::time::Instant::now();
    let ok = tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &hash)).await??;
    tracing::info!(
        verify_ms = verify_start.elapsed().as_millis(),
        "bcrypt verify done"
    );
    if !ok {
        bail!("Nomor HP atau kata sandi salah.");
    }

    let phone = user.phone_number.clone().unwrap_or_default();
    let token = jwt.sign(user.id, &user.full_name, &phone, &user.role)?;
    Ok(LoginOk {
        redirect: crate::models::role_home(&user.role).to_string(),
        user: SessionUser {
            id: user.id,
            name: user.full_name,
            role: user.role,
        },
        token,
    })
}

/// Forgot-password via WA: cari user dari nomor HP → buat password baru → kirim
/// lewat WhatsApp. Best-effort & anti-enumerasi: SELALU balas Ok (tak bocorkan
/// apakah nomor terdaftar). bcrypt di `spawn_blocking` (CPU-bound).
pub async fn forgot_password(
    pool: &Pool,
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    phone: &str,
) -> Result<()> {
    let phone = normalize_phone(phone);
    if phone.len() < 8 {
        return Ok(()); // input tak masuk akal → diam
    }
    let Some(user_id) = repo::find_by_phone(pool, &phone).await? else {
        return Ok(()); // tak terdaftar → diam (anti-enumerasi)
    };

    let new_pw = super::registration::generate_random_password();
    let pw = new_pw.clone();
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 10)).await??;
    repo::set_password_hash(pool, user_id, &hash).await?;

    let msg = format!(
        "🔑 *Reset Password PPM AFM*\nPassword baru Anda: *{new_pw}*\n\nMasuk dengan nomor HP + password ini, lalu segera ganti password di menu Profil."
    );
    // Gagal WA tak menggagalkan reset (password sudah diganti); log saja.
    if let Err(e) = super::registration::send_wa_text(http, waha, &phone, &msg).await {
        tracing::warn!("forgot_password: WA gagal ke {phone}: {e}");
    }
    Ok(())
}

/// Ganti kata sandi user yang sedang login: cocokkan sandi LAMA (bcrypt verify),
/// bila cocok simpan sandi BARU (bcrypt hash). bcrypt di `spawn_blocking`.
pub async fn change_password(pool: &Pool, user_id: i64, old: &str, new: &str) -> Result<()> {
    if new.chars().count() < 6 {
        bail!("Kata sandi baru minimal 6 karakter.");
    }
    if new == old {
        bail!("Kata sandi baru harus berbeda dari yang lama.");
    }
    let Some(hash) = repo::get_password_hash(pool, user_id).await? else {
        bail!("Akun tidak ditemukan.");
    };
    let old_s = old.to_string();
    let ok = tokio::task::spawn_blocking(move || bcrypt::verify(&old_s, &hash)).await??;
    if !ok {
        bail!("Kata sandi lama salah.");
    }
    let new_s = new.to_string();
    let new_hash = tokio::task::spawn_blocking(move || bcrypt::hash(&new_s, 10)).await??;
    repo::set_password_hash(pool, user_id, &new_hash).await?;
    Ok(())
}

/// Bootstrap: bila tabel users KOSONG, buat admin awal
/// (username `admin`, password dari env ADMIN_PASSWORD, default "admin123").
/// Tidak menyentuh apa pun bila sudah ada data (aman utk DB yang sedang diisi).
pub async fn ensure_seed_admin(pool: &Pool) -> Result<()> {
    if repo::count_users(pool).await? > 0 {
        return Ok(());
    }
    let pw = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".into());
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 10)).await??;
    repo::insert_admin(pool, &hash).await?;
    tracing::info!("Seed admin dibuat (username: admin — ganti password segera)");
    Ok(())
}
