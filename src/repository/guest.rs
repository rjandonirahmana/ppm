//! repository/guest.rs — Buku tamu (migrasi 35). Baris dibuat saat mesin IoT
//! berhasil check-in tamu (kode cocok di Redis + wajah terunggah).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub async fn insert_guest_visit(
    pool: &Pool,
    name: &str,
    phone: &str,
    purpose: &str,
    face_url: Option<&str>,
    device_id: Option<i64>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO guest_visits (name, phone, purpose, face_url, device_id) \
             VALUES ($1, $2, $3, $4, $5) RETURNING id",
            &[&name, &phone, &purpose, &face_url, &device_id],
        )
        .await
        .context("insert_guest_visit")?;
    Ok(row.get(0))
}

/// Satu kunjungan tamu untuk layar penjaga.
pub struct KunjunganTamu {
    pub id: i64,
    pub name: String,
    pub phone: String,
    pub purpose: String,
    pub face_url: Option<String>,
    pub checked_in_at: chrono::DateTime<chrono::Utc>,
    /// Nama penjaga yang memeriksa; None = belum diperiksa.
    pub verified_by_name: Option<String>,
    pub verified_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Catatan bila datanya janggal. Kosong = dinyatakan cocok.
    pub verify_note: String,
}

/// Kunjungan tamu terbaru dulu. `hanya_belum` = sisakan yang belum diperiksa.
///
/// Tabel ini sudah terisi sejak migrasi 35 tapi TAK PERNAH DIBACA — tak ada
/// satu pun layar yang menampilkannya. Fungsi inilah pembaca pertamanya, untuk
/// peran penjaga (migrasi 83).
pub async fn list_kunjungan_tamu(
    pool: &Pool,
    hanya_belum: bool,
    limit: i64,
) -> Result<Vec<KunjunganTamu>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT g.id, g.name, g.phone, g.purpose, g.face_url, g.checked_in_at, \
                    u.full_name, g.verified_at, COALESCE(g.verify_note, '') \
               FROM guest_visits g \
               LEFT JOIN users u ON u.id = g.verified_by \
              WHERE ($1::bool = FALSE OR g.verified_at IS NULL) \
              ORDER BY g.checked_in_at DESC \
              LIMIT $2",
            &[&hanya_belum, &limit],
        )
        .await
        .context("list_kunjungan_tamu")?;
    Ok(rows
        .into_iter()
        .map(|r| KunjunganTamu {
            id: r.get(0),
            name: r.get(1),
            phone: r.get(2),
            purpose: r.get(3),
            face_url: r.get(4),
            checked_in_at: r.get(5),
            verified_by_name: r.get(6),
            verified_at: r.get(7),
            verify_note: r.get(8),
        })
        .collect())
}

/// Tandai satu kunjungan sudah diperiksa penjaga.
///
/// `catatan` kosong = data dinyatakan COCOK. Terisi = ada yang janggal, dan
/// isinya itulah yang dibaca pengurus nanti.
///
/// Guard `verified_at IS NULL` membuatnya sekali jalan: dua penjaga yang
/// membuka layar bersamaan tak bisa saling menimpa catatan, dan yang kalah
/// mendapat `false` alih-alih diam-diam menghapus temuan rekannya.
pub async fn periksa_kunjungan_tamu(
    pool: &Pool,
    visit_id: i64,
    penjaga_id: i64,
    catatan: &str,
) -> Result<bool> {
    let catatan = catatan.trim();
    let note: Option<&str> = (!catatan.is_empty()).then_some(catatan);
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE guest_visits \
                SET verified_by = $2, verified_at = NOW(), verify_note = $3 \
              WHERE id = $1 AND verified_at IS NULL",
            &[&visit_id, &penjaga_id, &note],
        )
        .await
        .context("periksa_kunjungan_tamu")?;
    Ok(n > 0)
}
