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
    pub campus: Option<String>,
    pub major: Option<String>,
    pub gender: Option<String>,
}

/// Data profil lengkap satu user.
pub async fn profil_row(pool: &Pool, user_id: i64) -> Result<Option<ProfilRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT full_name, username, role, email, phone_number, address, nis, points, \
                    campus, major, gender \
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
        campus: r.get(8),
        major: r.get(9),
        gender: r.get(10),
    }))
}

/// Ubah profil mahasiswa (kampus/jurusan/gender) — santri sendiri (migrasi 26).
pub async fn update_profile_extra(
    pool: &Pool,
    user_id: i64,
    campus: Option<&str>,
    major: Option<&str>,
    gender: Option<&str>,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE users SET campus = $2, major = $3, gender = $4 WHERE id = $1",
        &[&user_id, &campus, &major, &gender],
    )
    .await
    .context("update_profile_extra")?;
    Ok(())
}

/// Riwayat IPK satu santri (terbaru dulu). Return (id, semester, ipk).
pub async fn list_ipk(pool: &Pool, user_id: i64) -> Result<Vec<(i64, String, f64)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, semester, ipk FROM ipk_history WHERE user_id = $1 ORDER BY id DESC",
            &[&user_id],
        )
        .await
        .context("list_ipk")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1), r.get(2))).collect())
}

pub async fn add_ipk(pool: &Pool, user_id: i64, semester: &str, ipk: f64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO ipk_history (user_id, semester, ipk) VALUES ($1, $2, $3) RETURNING id",
            &[&user_id, &semester, &ipk],
        )
        .await
        .context("add_ipk")?;
    Ok(row.get(0))
}

/// Hapus entri IPK — hanya milik user (guard user_id).
pub async fn delete_ipk(pool: &Pool, user_id: i64, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM ipk_history WHERE id = $1 AND user_id = $2", &[&id, &user_id])
        .await
        .context("delete_ipk")?;
    Ok(n > 0)
}

/// Reset saldo poin SEMUA santri ke nilai awal semester (PRD: 300). Return
/// jumlah santri ter-reset. Mencatat log per santri agar jejak reset terlihat.
pub async fn reset_semester_points(pool: &Pool, start: i32) -> Result<i64> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "WITH r AS ( \
                UPDATE users SET points = $1, updated_at = NOW() \
                WHERE role = 'santri' RETURNING id, points \
             ), lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category) \
                SELECT id, 0, 'Reset saldo poin awal semester', 'other' FROM r \
             ) SELECT 1",
            &[&start],
        )
        .await
        .context("reset_semester_points")?;
    // execute() untuk statement diakhiri SELECT tak mengembalikan rowcount UPDATE;
    // hitung santri terpisah agar akurat.
    let _ = n;
    let row = c
        .query_one("SELECT COUNT(*) FROM users WHERE role = 'santri'", &[])
        .await
        .context("reset_semester_points count")?;
    Ok(row.get(0))
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

// ── User Control (admin, migrasi 17) ─────────────────────────────────────────

pub struct UserListRow {
    pub id: i64,
    pub full_name: String,
    pub role: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub nis: Option<String>,
    pub is_active: bool,
}

/// Daftar user, opsional difilter per peran. Terurut peran lalu nama.
pub async fn list_users(pool: &Pool, role_filter: Option<&str>, limit: i64) -> Result<Vec<UserListRow>> {
    let c = pool.get().await?;
    let rows = match role_filter {
        Some(r) => {
            c.query(
                "SELECT id, full_name, role, email, username, nis, is_active FROM users \
                 WHERE role = $1 ORDER BY full_name LIMIT $2",
                &[&r, &limit],
            )
            .await
        }
        None => {
            c.query(
                "SELECT id, full_name, role, email, username, nis, is_active FROM users \
                 ORDER BY role, full_name LIMIT $1",
                &[&limit],
            )
            .await
        }
    }
    .context("list_users")?;
    Ok(rows
        .into_iter()
        .map(|r| UserListRow {
            id: r.get(0),
            full_name: r.get(1),
            role: r.get(2),
            email: r.get(3),
            username: r.get(4),
            nis: r.get(5),
            is_active: r.get(6),
        })
        .collect())
}

/// (total, santri, staff [guru+dewan_guru+pamong], nonaktif).
pub async fn user_counts(pool: &Pool) -> Result<(i64, i64, i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*), \
                COUNT(*) FILTER (WHERE role = 'santri'), \
                COUNT(*) FILTER (WHERE role IN ('teacher','dewan_guru','supervisor')), \
                COUNT(*) FILTER (WHERE NOT is_active) \
             FROM users",
            &[],
        )
        .await
        .context("user_counts")?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub async fn set_active(pool: &Pool, user_id: i64, active: bool) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET is_active = $2, updated_at = NOW() WHERE id = $1",
            &[&user_id, &active],
        )
        .await
        .context("set_active")?;
    Ok(n > 0)
}

const VALID_ROLES: &[&str] =
    &["admin", "teacher", "dewan_guru", "supervisor", "santri", "parent"];

// ── Registrasi via link undangan (migrasi 19) ───────────────────────────────

/// Cek nomor HP sudah terdaftar atau belum (guard duplikat saat registrasi).
pub async fn find_by_phone(pool: &Pool, phone: &str) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT id FROM users WHERE phone_number = $1", &[&phone])
        .await
        .context("find_by_phone")?;
    Ok(row.map(|r| r.get(0)))
}

/// Buat user dari alur registrasi (name+phone saja — NIS/username/email diisi
/// admin belakangan lewat /students atau /kontrol-pengguna, sama seperti akun
/// lain yang dikelola admin).
pub async fn insert_registered_user(
    pool: &Pool,
    name: &str,
    phone: &str,
    role: &str,
    password_hash: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO users (full_name, phone_number, role, password_hash) \
             VALUES ($1, $2, $3, $4) RETURNING id",
            &[&name, &phone, &role, &password_hash],
        )
        .await
        .context("insert_registered_user")?;
    Ok(row.get(0))
}

pub async fn set_role(pool: &Pool, user_id: i64, role: &str) -> Result<bool> {
    if !VALID_ROLES.contains(&role) {
        return Ok(false);
    }
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET role = $2, updated_at = NOW() WHERE id = $1",
            &[&user_id, &role],
        )
        .await
        .context("set_role")?;
    Ok(n > 0)
}
