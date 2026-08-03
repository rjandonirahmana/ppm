//! service/admin.rs — Halaman "User Control" (admin-only, migrasi 17): daftar
//! user + statistik, aktif/nonaktifkan akun, ganti peran — semua aksi tercatat
//! ke activity_logs.

use anyhow::Result;
use deadpool_postgres::Pool;

use super::fmt::fmt_ago;
use crate::models::{ActivityLogItem, RfidDeviceItem, UserControlData, UserRow};
use crate::repository as repo;

/// Saldo poin awal semester (PRD "Sistem Poin 2.0": 300 poin).
pub const SEMESTER_START_POINTS: i32 = 300;

/// Reset saldo poin semua santri ke 300 (awal semester baru, PRD). Return
/// jumlah santri ter-reset.
pub async fn reset_semester_points(pool: &Pool) -> Result<i64> {
    repo::reset_semester_points(pool, SEMESTER_START_POINTS).await
}

/// api_key acak (32 hex) untuk perangkat RFID baru.
/// SHA-256 hex dari api_key perangkat.
///
/// Hash CEPAT, bukan bcrypt: fungsi ini dipanggil pada SETIAP tap kartu, dan
/// bcrypt yang sengaja lambat (~80 ms) akan membuat mesin absensi tersendat.
///
/// BATAS PERLINDUNGANNYA JUJUR: kunci 16 digit = ~53 bit, jadi bila dump DB
/// bocor, hash-nya bisa dibongkar dengan tenaga GPU dalam hitungan bulan. Yang
/// dicegah di sini adalah kebocoran DB langsung menyerahkan kunci yang SIAP
/// PAKAI. Bila sebuah kunci dicurigai bocor, ganti lewat tombol regenerasi —
/// jauh lebih murah daripada memperpanjang kunci dan menyulitkan pengetikan.
pub fn hash_api_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(key.trim().as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// api_key perangkat RFID — DIGIT SAJA, 16 angka (migrasi 49). Dulu 32 hex;
/// diubah karena kunci ini diketik manual di captive portal firmware ESP8266,
/// dan huruf hex mudah keliru (0/O, b/6). 16 digit ≈ 53 bit: ruang 10^16 masih
/// jauh di luar jangkauan tebak-tebakan lewat jaringan.
///
/// Digit pertama dijaga bukan 0 supaya kunci tak terpotong bila ada firmware /
/// spreadsheet yang memperlakukannya sebagai bilangan.
fn gen_api_key() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut key = String::with_capacity(16);
    key.push(char::from_digit(rng.random_range(1..=9), 10).expect("1..9 valid"));
    for _ in 1..16 {
        key.push(char::from_digit(rng.random_range(0..=9), 10).expect("0..9 valid"));
    }
    key
}

/// Validasi kategori perangkat terhadap daftar sah (= CHECK constraint DB).
/// Kosong → 'custom' (perilaku absensi kelas biasa).
fn norm_category(c: &str) -> Result<String> {
    let c = c.trim();
    if c.is_empty() {
        return Ok("custom".to_string());
    }
    if !crate::models::DEVICE_CATEGORIES.iter().any(|(v, _)| *v == c) {
        bail_user!("Kategori perangkat tidak dikenal.");
    }
    Ok(c.to_string())
}

/// Daftar perangkat RFID (ruang) untuk manajemen admin + dropdown jadwal.
pub async fn rfid_devices(pool: &Pool) -> Result<Vec<RfidDeviceItem>> {
    Ok(repo::list_devices(pool)
        .await?
        .into_iter()
        .map(|d| RfidDeviceItem {
            id: d.id,
            device_name: d.device_name,
            serial_number: d.serial_number.unwrap_or_default(),
            location: d.location.unwrap_or_default(),
            api_key: d.api_key,
            category: d.category,
        })
        .collect())
}

/// Buat perangkat RFID. `api_key` kosong → di-generate. Return (id, api_key)
/// — kunci HANYA dikembalikan di sini; setelah ini hanya hash-nya yang tersimpan.
pub async fn create_rfid_device(
    pool: &Pool,
    device_name: &str,
    serial_number: &str,
    location: &str,
    api_key: &str,
    category: &str,
) -> Result<(i64, String)> {
    let name = device_name.trim();
    if name.is_empty() {
        bail_user!("Nama perangkat/ruang wajib diisi.");
    }
    let serial = serial_number.trim();
    let loc = location.trim();
    let key = api_key.trim();
    let key = if key.is_empty() { gen_api_key() } else { key.to_string() };
    let cat = norm_category(category)?;
    // Kembalikan kuncinya: sejak disimpan sebagai hash (migrasi 53), inilah
    // SATU-SATUNYA kesempatan admin melihatnya. Tak dikembalikan = perangkat
    // baru tak bisa dikonfigurasi tanpa langsung menggantinya.
    let id = repo::create_device(
        pool,
        name,
        (!serial.is_empty()).then_some(serial),
        (!loc.is_empty()).then_some(loc),
        &key,
        &cat,
    )
    .await?;
    Ok((id, key))
}

pub async fn update_rfid_device(
    pool: &Pool,
    id: i64,
    device_name: &str,
    serial_number: &str,
    location: &str,
    category: &str,
) -> Result<()> {
    let name = device_name.trim();
    if name.is_empty() {
        bail_user!("Nama perangkat/ruang wajib diisi.");
    }
    let serial = serial_number.trim();
    let loc = location.trim();
    let cat = norm_category(category)?;
    if !repo::update_device(
        pool,
        id,
        name,
        (!serial.is_empty()).then_some(serial),
        (!loc.is_empty()).then_some(loc),
        &cat,
    )
    .await?
    {
        bail_user!("Perangkat tidak ditemukan.");
    }
    Ok(())
}

/// Ganti api_key perangkat (mis. bila bocor) → return api_key baru.
pub async fn regenerate_rfid_key(pool: &Pool, id: i64) -> Result<String> {
    let key = gen_api_key();
    if !repo::set_api_key(pool, id, &key).await? {
        bail_user!("Perangkat tidak ditemukan.");
    }
    Ok(key)
}

pub async fn delete_rfid_device(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_device(pool, id).await? {
        bail_user!("Perangkat tidak ditemukan.");
    }
    Ok(())
}

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
        bail_user!("Tidak bisa mengubah status akun sendiri.");
    }
    if !repo::set_active(pool, target_id, active).await? {
        bail_user!("Pengguna tidak ditemukan.");
    }
    let action = if active { "user.activate" } else { "user.deactivate" };
    let _ = repo::insert_log(pool, actor_id, Some(target_id), action, None).await;
    Ok(())
}

pub async fn change_role(pool: &Pool, actor_id: i64, target_id: i64, new_role: &str) -> Result<()> {
    if actor_id == target_id {
        bail_user!("Tidak bisa mengubah peran akun sendiri.");
    }
    if !repo::set_role(pool, target_id, new_role).await? {
        bail_user!("Peran tidak valid atau pengguna tidak ditemukan.");
    }
    let detail = format!("Peran baru: {}", role_label(new_role));
    let _ = repo::insert_log(pool, actor_id, Some(target_id), "user.role_change", Some(&detail)).await;
    Ok(())
}
