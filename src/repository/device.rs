//! repository/device.rs — Query tabel rfid_devices (perangkat/ruang RFID).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub struct DeviceRow {
    pub id: i64,
    pub device_name: String,
    pub location: Option<String>,
    /// Menentukan PERILAKU tap (migrasi 49): 'gate_utama' → keluar/masuk area
    /// pondok; selainnya → absensi kelas.
    pub category: String,
}

/// Cari perangkat dari api_key. Yang dibandingkan adalah HASH-nya (migrasi 53)
/// — kolom plaintext sudah tak ada. Pemanggil tetap mengirim kunci apa adanya;
/// hashing dilakukan di sini supaya tak ada jalur yang lupa melakukannya.
pub async fn find_device_by_key(pool: &Pool, api_key: &str) -> Result<Option<DeviceRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, device_name, location, category FROM rfid_devices \
             WHERE api_key_hash = $1",
            &[&crate::service::admin::hash_api_key(api_key)],
        )
        .await?;
    Ok(row.map(|r| DeviceRow {
        id: r.get(0),
        device_name: r.get(1),
        location: r.get(2),
        category: r.get(3),
    }))
}

/// Perangkat RFID lengkap (untuk manajemen admin + dropdown ruang jadwal).
pub struct DeviceFull {
    pub id: i64,
    pub device_name: String,
    pub serial_number: Option<String>,
    pub location: Option<String>,
    pub api_key: String,
    pub category: String,
}

pub async fn list_devices(pool: &Pool) -> Result<Vec<DeviceFull>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, device_name, serial_number, location, '', category \
             FROM rfid_devices ORDER BY category, device_name",
            &[],
        )
        .await
        .context("list_devices")?;
    Ok(rows
        .into_iter()
        .map(|r| DeviceFull {
            id: r.get(0),
            device_name: r.get(1),
            serial_number: r.get(2),
            location: r.get(3),
            api_key: r.get(4),
            category: r.get(5),
        })
        .collect())
}

pub async fn create_device(
    pool: &Pool,
    device_name: &str,
    serial_number: Option<&str>,
    location: Option<&str>,
    api_key: &str,
    category: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO rfid_devices \
                (device_name, serial_number, location, api_key_hash, category) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[
                &device_name,
                &serial_number,
                &location,
                &crate::service::admin::hash_api_key(api_key),
                &category,
            ],
        )
        .await
        .context("create_device")?;
    Ok(row.get(0))
}

pub async fn update_device(
    pool: &Pool,
    id: i64,
    device_name: &str,
    serial_number: Option<&str>,
    location: Option<&str>,
    category: &str,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE rfid_devices SET device_name = $2, serial_number = $3, location = $4, \
                    category = $5 \
             WHERE id = $1",
            &[&id, &device_name, &serial_number, &location, &category],
        )
        .await
        .context("update_device")?;
    Ok(n > 0)
}

/// Ganti api_key (mis. bila bocor). Return true bila ada baris ter-update.
pub async fn set_api_key(pool: &Pool, id: i64, api_key: &str) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE rfid_devices SET api_key_hash = $2 WHERE id = $1",
            &[&id, &crate::service::admin::hash_api_key(api_key)],
        )
        .await
        .context("set_api_key")?;
    Ok(n > 0)
}

pub async fn delete_device(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM rfid_devices WHERE id = $1", &[&id])
        .await
        .context("delete_device")?;
    Ok(n > 0)
}

/// true bila perangkat ini GERBANG UTAMA. Dipakai validasi jadwal: gerbang
/// utama tak boleh jadi ruang kelas (lihat repository::kelas::device_options).
pub async fn is_gate_device(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT category FROM rfid_devices WHERE id = $1", &[&id])
        .await
        .context("is_gate_device")?;
    Ok(row
        .map(|r| crate::models::is_main_gate(&r.get::<_, String>(0)))
        .unwrap_or(false))
}

/// Isi `api_key_hash` untuk perangkat yang masih menyimpan kunci plaintext.
///
/// Dijalankan sekali saat start (main.rs). Backfill TIDAK bisa dilakukan di SQL
/// tanpa ekstensi `pgcrypto` — hash-nya harus dihitung dengan fungsi yang sama
/// dengan yang dipakai saat lookup, dan itu ada di Rust.
///
/// Setelah semua terisi, kolom plaintext bisa di-drop (migrasi 53 bagian 2).
/// Return jumlah baris yang diisi.
pub async fn backfill_api_key_hashes(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    // Kolom plaintext mungkin SUDAH di-drop → bukan galat, cukup lewati.
    let has_plain = c
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM information_schema.columns \
              WHERE table_name = 'rfid_devices' AND column_name = 'api_key')",
            &[],
        )
        .await
        .context("backfill: cek kolom")?
        .get::<_, bool>(0);
    if !has_plain {
        return Ok(0);
    }

    let rows = c
        .query(
            "SELECT id, api_key FROM rfid_devices \
              WHERE api_key IS NOT NULL AND api_key <> '' AND api_key_hash IS NULL",
            &[],
        )
        .await
        .context("backfill: ambil plaintext")?;

    let mut n = 0i64;
    for r in rows {
        let id: i64 = r.get(0);
        let plain: String = r.get(1);
        c.execute(
            "UPDATE rfid_devices SET api_key_hash = $2 WHERE id = $1",
            &[&id, &crate::service::admin::hash_api_key(&plain)],
        )
        .await
        .context("backfill: tulis hash")?;
        n += 1;
    }
    Ok(n)
}
