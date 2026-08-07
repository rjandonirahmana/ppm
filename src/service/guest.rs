//! service/guest.rs — Buku tamu berbasis KODE di Redis (migrasi 35).
//!
//! Alur:
//!   1. Tamu isi /tamu → `register_guest` → kode 6-digit disimpan
//!      `tamu:code:{kode}` = JSON data tamu (TTL 12 jam ≈ "hari itu").
//!   2. Tamu ketik kode di mesin IoT → mesin kirim {api_key, code, foto} →
//!      `consume_guest` cari kode → hapus → (handler) simpan kunjungan+wajah →
//!      `mark_done` set `tamu:done:{kode}` (TTL 10 mnt) untuk polling HP.
//!   3. Halaman /tamu polling `check_status` → tampil ✅ + wajah saat done.

use anyhow::Result;
use rand::RngExt;
use redis::{aio::ConnectionManager, AsyncCommands};
use serde::{Deserialize, Serialize};

use crate::models::GuestCheckin;

/// TTL kode tamu: 12 jam (cukup untuk satu hari kunjungan).
const CODE_TTL: u64 = 12 * 3600;
/// TTL penanda "sukses" agar HP tamu sempat polling & lihat ✅.
const DONE_TTL: u64 = 600;

#[derive(Serialize, Deserialize)]
pub struct PendingGuest {
    pub name: String,
    pub phone: String,
    pub purpose: String,
}

fn code_key(code: &str) -> String {
    format!("tamu:code:{code}")
}
fn done_key(code: &str) -> String {
    format!("tamu:done:{code}")
}

fn gen_code() -> String {
    format!("{:06}", rand::rng().random_range(0..=999_999))
}

/// Daftarkan tamu → kode unik 6-digit. Retry bila tabrakan kode.
pub async fn register_guest(
    redis: &mut ConnectionManager,
    name: &str,
    phone: &str,
    purpose: &str,
) -> Result<String> {
    let name = name.trim();
    let phone = phone.trim();
    if name.is_empty() || phone.len() < 6 {
        bail_user!("Nama dan nomor HP wajib diisi dengan benar.");
    }
    let g = PendingGuest {
        name: name.to_string(),
        phone: phone.to_string(),
        purpose: purpose.trim().to_string(),
    };
    let json = serde_json::to_string(&g)?;
    for _ in 0..12 {
        let code = gen_code();
        let exists: bool = redis.exists(code_key(&code)).await.unwrap_or(false);
        if exists {
            continue;
        }
        let _: () = redis.set_ex(code_key(&code), &json, CODE_TTL).await?;
        return Ok(code);
    }
    bail_user!("Gagal membuat kode unik, coba lagi.");
}

/// Cari kode → data tamu, lalu HAPUS kode (sekali pakai). None = tak ada/kadaluarsa.
pub async fn consume_guest(
    redis: &mut ConnectionManager,
    code: &str,
) -> Result<Option<PendingGuest>> {
    // GETDEL, bukan GET lalu DEL: kode tamu sekali pakai, dan dua mesin yang
    // memindai kode yang sama pada saat bersamaan sama-sama lolos GET sebelum
    // salah satunya sempat DEL — keduanya lalu tercatat check-in dengan kode
    // yang sama. GETDEL menyatukan ambil-dan-hapus jadi satu operasi atomik di
    // sisi Redis, jadi hanya satu pemanggil yang bisa menang.
    let json: Option<String> = redis::cmd("GETDEL")
        .arg(code_key(code))
        .query_async(redis)
        .await?;
    let Some(json) = json else {
        return Ok(None);
    };
    Ok(Some(serde_json::from_str(&json)?))
}

/// Tandai kode sukses check-in (untuk polling HP tamu).
pub async fn mark_done(
    redis: &mut ConnectionManager,
    code: &str,
    checkin: &GuestCheckin,
) -> Result<()> {
    let json = serde_json::to_string(checkin)?;
    let _: () = redis.set_ex(done_key(code), json, DONE_TTL).await?;
    Ok(())
}

/// Status untuk halaman /tamu: Some = sudah di-check-in mesin (tampil ✅).
pub async fn check_status(
    redis: &mut ConnectionManager,
    code: &str,
) -> Result<Option<GuestCheckin>> {
    let json: Option<String> = redis.get(done_key(code)).await?;
    match json {
        Some(j) => Ok(Some(serde_json::from_str(&j)?)),
        None => Ok(None),
    }
}
