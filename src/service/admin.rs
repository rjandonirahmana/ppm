//! service/admin.rs — Halaman "User Control" (admin-only, migrasi 17): daftar
//! user + statistik, aktif/nonaktifkan akun, ganti peran — semua aksi tercatat
//! ke activity_logs.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use super::fmt::fmt_ago;
use crate::models::{ActivityLogItem, UserControlData, UserRow};
use crate::repository as repo;

fn role_label(role: &str) -> &'static str {
    match role {
        "admin" => "Admin",
        "teacher" => "Guru",
        "dewan_guru" => "Dewan Guru",
        "supervisor" => "Pamong",
        "santri" => "Santri",
        "parent" => "Orang Tua",
        _ => "Pengguna",
    }
}

fn action_label(action: &str) -> String {
    match action {
        "user.activate" => "Aktifkan Akun".into(),
        "user.deactivate" => "Nonaktifkan Akun".into(),
        "user.role_change" => "Ganti Peran".into(),
        other => other.into(),
    }
}

pub async fn user_control_data(pool: &Pool, role_filter: Option<&str>) -> Result<UserControlData> {
    let (counts, rows) = tokio::join!(
        repo::user_counts(pool),
        repo::list_users(pool, role_filter, 500),
    );
    let (total, santri_count, staff_count, inactive_count) = counts?;
    let users = rows?
        .into_iter()
        .map(|u| {
            let contact = if u.role == "santri" {
                u.nis.unwrap_or_default()
            } else {
                u.email.or(u.username).unwrap_or_default()
            };
            UserRow {
                id: u.id,
                name: u.full_name,
                role_label: role_label(&u.role).into(),
                role: u.role,
                contact,
                is_active: u.is_active,
            }
        })
        .collect();

    Ok(UserControlData { total, santri_count, staff_count, inactive_count, users })
}

pub async fn recent_activity(pool: &Pool, limit: i64) -> Result<Vec<ActivityLogItem>> {
    Ok(repo::recent_logs(pool, limit)
        .await?
        .into_iter()
        .map(|l| ActivityLogItem {
            actor_name: l.actor_name.unwrap_or_else(|| "Sistem".into()),
            target_name: l.target_name,
            action_label: action_label(&l.action),
            detail: l.detail,
            when_label: fmt_ago(l.created_at),
        })
        .collect())
}

pub async fn toggle_active(pool: &Pool, actor_id: i64, target_id: i64, active: bool) -> Result<()> {
    if actor_id == target_id {
        bail!("Tidak bisa mengubah status akun sendiri.");
    }
    if !repo::set_active(pool, target_id, active).await? {
        bail!("Pengguna tidak ditemukan.");
    }
    let action = if active { "user.activate" } else { "user.deactivate" };
    let _ = repo::insert_log(pool, actor_id, Some(target_id), action, None).await;
    Ok(())
}

pub async fn change_role(pool: &Pool, actor_id: i64, target_id: i64, new_role: &str) -> Result<()> {
    if actor_id == target_id {
        bail!("Tidak bisa mengubah peran akun sendiri.");
    }
    if !repo::set_role(pool, target_id, new_role).await? {
        bail!("Peran tidak valid atau pengguna tidak ditemukan.");
    }
    let detail = format!("Peran baru: {}", role_label(new_role));
    let _ = repo::insert_log(pool, actor_id, Some(target_id), "user.role_change", Some(&detail)).await;
    Ok(())
}
