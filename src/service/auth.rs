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

/// Verifikasi kredensial → JWT (pola sama e-ticketing AuthService::login).
/// bcrypt CPU-bound → `spawn_blocking` agar tidak menyumbat worker async.
pub async fn login(pool: &Pool, jwt: &JwtService, login: &str, password: &str) -> Result<LoginOk> {
    let Some(user) = repo::find_user_for_login(pool, login.trim()).await? else {
        bail!("Username/email atau kata sandi salah.");
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
        bail!("Username/email atau kata sandi salah.");
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
