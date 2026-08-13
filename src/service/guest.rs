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
    if name.is_empty() {
        bail_user!("Nama wajib diisi.");
    }
    // Nomor tamu DINORMALKAN seperti nomor lain di aplikasi ini. Sebelumnya ia
    // disimpan mentah — hanya `trim()` — sehingga tamu yang menulis "0812…"
    // tersimpan apa adanya. Bentuk itu bukan chat-id WAHA yang sah, jadi pesan
    // apa pun ke tamu mustahil terkirim, dan nomor yang sama tak akan cocok
    // dengan catatan mana pun yang tersimpan dalam bentuk `628…`.
    //
    // Panjangnya pun tak lagi ditebak dari `len() < 6`: pemeriksaan itu
    // meloloskan "123456" sebagai nomor HP yang sah.
    let Some(phone) = crate::models::normalisasi_hp(phone) else {
        bail_user!("{}", crate::models::pesan_hp_tidak_sah());
    };
    let g = PendingGuest {
        name: name.to_string(),
        phone,
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

// ── Layar penjaga: tinjau kunjungan tamu (migrasi 83) ────────────────────────

/// Daftar kunjungan tamu untuk penjaga.
///
/// `hanya_belum` menyisakan yang belum diperiksa — itu pekerjaan penjaga.
/// Riwayat lengkap tetap bisa dibuka untuk menelusuri kunjungan lama.
pub const TAMU_PER_PAGE: i64 = 20;

/// Rentang waktu → (batas paling awal, label untuk layar).
///
/// "Semester ini" dibaca dari tabel semester akademik (migrasi 40), bukan
/// ditebak enam bulan ke belakang: pondok yang memundurkan awal semester akan
/// melihat angka yang salah sepanjang periode itu, dan tak ada yang menyadari
/// karena angkanya tetap tampak masuk akal.
async fn batas_rentang(
    pool: &deadpool_postgres::Pool,
    rentang: &str,
) -> (Option<chrono::DateTime<chrono::Utc>>, String) {
    match rentang {
        "hari_ini" => (Some(super::fmt::days_ago_wib(0)), "Hari ini".into()),
        "7" => (Some(super::fmt::days_ago_wib(6)), "7 hari terakhir".into()),
        "30" => (Some(super::fmt::days_ago_wib(29)), "30 hari terakhir".into()),
        "semester" => match super::santri::current_semester(pool).await {
            Ok((mulai, label)) => (Some(mulai), label),
            Err(_) => (None, "Semua".into()),
        },
        // Termasuk "semua" dan nilai tak dikenal: JANGAN diam-diam menyaring
        // sesuatu yang tak diminta.
        _ => (None, "Semua".into()),
    }
}

fn baris_tamu(g: crate::repository::KunjunganTamu) -> crate::models::TamuMasukItem {
    crate::models::TamuMasukItem {
        id: g.id,
        name: g.name,
        phone: g.phone,
        purpose: g.purpose,
        face_url: g.face_url.unwrap_or_default(),
        waktu_label: super::fmt::fmt_when(g.checked_in_at),
        diperiksa: g.verified_at.is_some(),
        diperiksa_oleh: g.verified_by_name.unwrap_or_default(),
        catatan: g.verify_note,
    }
}

pub async fn tamu_masuk(
    pool: &deadpool_postgres::Pool,
    hanya_belum: bool,
    rentang: &str,
) -> Result<crate::models::TamuMasukData> {
    let (sejak, rentang_label) = batas_rentang(pool, rentang).await;
    let (rows, total, belum) = tokio::join!(
        crate::repository::list_kunjungan_tamu(pool, hanya_belum, sejak, TAMU_PER_PAGE, 0),
        crate::repository::count_kunjungan_tamu(pool, hanya_belum, sejak),
        // Dihitung terpisah dari daftar: saat riwayat lengkap dibuka, angka
        // "menunggu diperiksa" harus tetap menyebut yang menunggu — bukan
        // jumlah baris yang kebetulan sedang tampil.
        crate::repository::count_kunjungan_tamu(pool, true, sejak),
    );
    Ok(crate::models::TamuMasukData {
        belum_diperiksa: belum?,
        total: total?,
        rentang_label,
        items: rows?.into_iter().map(baris_tamu).collect(),
    })
}

/// Halaman berikutnya (gulir-tak-berujung).
pub async fn tamu_masuk_page(
    pool: &deadpool_postgres::Pool,
    hanya_belum: bool,
    rentang: &str,
    offset: i64,
) -> Result<Vec<crate::models::TamuMasukItem>> {
    let (sejak, _) = batas_rentang(pool, rentang).await;
    let rows = crate::repository::list_kunjungan_tamu(
        pool,
        hanya_belum,
        sejak,
        TAMU_PER_PAGE,
        offset.max(0),
    )
    .await?;
    Ok(rows.into_iter().map(baris_tamu).collect())
}

/// Tandai satu kunjungan sudah diperiksa. Catatan kosong = data cocok.
pub async fn periksa_tamu(
    pool: &deadpool_postgres::Pool,
    visit_id: i64,
    penjaga_id: i64,
    catatan: &str,
) -> Result<()> {
    // Batas panjang catatan: kolomnya TEXT, dan kotak catatan yang tak berbatas
    // adalah tempat orang menempelkan apa saja.
    let catatan: String = catatan.chars().take(300).collect();
    if !crate::repository::periksa_kunjungan_tamu(pool, visit_id, penjaga_id, &catatan).await? {
        bail_user!("Kunjungan ini sudah diperiksa orang lain, atau tidak ditemukan.");
    }
    Ok(())
}
