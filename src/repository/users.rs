//! repository/users.rs — Query tabel users.

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub struct LoginRow {
    pub id: i64,
    pub full_name: String,
    pub role: String,
    pub password_hash: String,
    pub phone_number: Option<String>,
}

/// Cari user untuk login — cocokkan username ATAU email ATAU NIS.
pub async fn find_user_for_login(pool: &Pool, login: &str) -> Result<Option<LoginRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, full_name, role, password_hash, phone_number FROM users \
             WHERE (username = $1 OR email = $1 OR nis = $1) AND is_active = TRUE",
            &[&login],
        )
        .await
        .context("find_user_for_login")?;
    Ok(row.map(|r| LoginRow {
        id: r.get(0),
        full_name: r.get(1),
        role: r.get(2),
        password_hash: r.get(3),
        phone_number: r.get(4),
    }))
}

pub struct UserHome {
    pub full_name: String,
    pub points: i32,
}

pub async fn user_home(pool: &Pool, user_id: i64) -> Result<Option<UserHome>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT full_name, points FROM users WHERE id = $1", &[&user_id])
        .await?;
    Ok(row.map(|r| UserHome {
        full_name: r.get(0),
        points: r.get(1),
    }))
}

/// Cari user dari nomor kartu RFID.
pub async fn find_user_by_card(pool: &Pool, card: i64) -> Result<Option<(i64, String)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, full_name FROM users WHERE rfid_cards = $1 AND is_active = TRUE",
            &[&card],
        )
        .await?;
    Ok(row.map(|r| (r.get(0), r.get(1))))
}

pub struct ProfilRow {
    pub full_name: String,
    pub username: Option<String>,
    pub role: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub address: Option<String>,
    pub nis: Option<String>,
    pub points: i32,
}

/// Data profil lengkap satu user.
pub async fn profil_row(pool: &Pool, user_id: i64) -> Result<Option<ProfilRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT full_name, username, role, email, phone_number, address, nis, points \
             FROM users WHERE id = $1",
            &[&user_id],
        )
        .await
        .context("profil_row")?;
    Ok(row.map(|r| ProfilRow {
        full_name: r.get(0),
        username: r.get(1),
        role: r.get(2),
        email: r.get(3),
        phone_number: r.get(4),
        address: r.get(5),
        nis: r.get(6),
        points: r.get(7),
    }))
}

/// Jumlah user (dipakai bootstrap seed).
pub async fn count_users(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c.query_one("SELECT COUNT(*) FROM users", &[]).await?;
    Ok(row.get(0))
}

/// Buat user admin awal (bootstrap saat tabel kosong).
pub async fn insert_admin(pool: &Pool, hash: &str) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO users (username, email, full_name, password_hash, role) \
         VALUES ('admin', 'admin@ppmafm.sch.id', 'Administrator', $1, 'admin')",
        &[&hash],
    )
    .await
    .context("insert_admin")?;
    Ok(())
}
