//! repository/sarana.rs — Query sarana & prasarana.
//!
//! Skema & alasan bentuknya ada di `migration/94_sarana_prasarana.sql`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

pub struct SaranaRow {
    pub id: i64,
    pub nama: String,
    pub kategori: String,
    pub lokasi: String,
    pub jumlah: i32,
    pub kondisi: String,
    pub catatan: String,
    pub updated_at: DateTime<Utc>,
}

/// Daftar sarana, boleh disaring kategori dan/atau kondisi.
///
/// Kedua penyaring `Option`, dan keduanya dikirim sebagai parameter dengan pola
/// `($n IS NULL OR kolom = $n)` alih-alih merangkai SQL berbeda per kombinasi.
/// Empat kombinasi berarti empat query yang harus sama-sama benar; satu query
/// berarti satu yang harus benar, dan Postgres tetap memakai indexnya karena
/// cabang `IS NULL` diputuskan saat perencanaan.
pub async fn list_sarana(
    pool: &Pool,
    kategori: Option<&str>,
    kondisi: Option<&str>,
    limit: i64,
) -> Result<Vec<SaranaRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, nama, kategori, lokasi, jumlah, kondisi, catatan, updated_at \
               FROM sarana \
              WHERE ($1::text IS NULL OR kategori = $1) \
                AND ($2::text IS NULL OR kondisi  = $2) \
              ORDER BY nama \
              LIMIT $3",
            &[&kategori, &kondisi, &limit],
        )
        .await
        .context("list_sarana")?;
    Ok(rows
        .into_iter()
        .map(|r| SaranaRow {
            id: r.get(0),
            nama: r.get(1),
            kategori: r.get(2),
            lokasi: r.get(3),
            jumlah: r.get(4),
            kondisi: r.get(5),
            catatan: r.get(6),
            updated_at: r.get(7),
        })
        .collect())
}

/// Ringkasan: jenis barang, total unit, dan berapa unit yang perlu perbaikan.
///
/// SATU query, bukan tiga. Ketiganya membaca tabel yang sama dan dibaca
/// bersamaan di kepala halaman; memisahkannya berarti tiga perjalanan ke
/// database untuk satu baris jawaban.
///
/// `COALESCE` pada penjumlahan: `SUM` atas tabel kosong menghasilkan NULL, dan
/// pondok yang belum mendata apa pun harus melihat angka 0 — bukan galat
/// konversi tipe.
pub async fn ringkas_sarana(pool: &Pool) -> Result<(i64, i64, i64)> {
    let c = pool.get().await?;
    let r = c
        .query_one(
            "SELECT COUNT(*)::bigint, \
                    COALESCE(SUM(jumlah), 0)::bigint, \
                    COALESCE(SUM(jumlah) FILTER (WHERE kondisi <> 'baik'), 0)::bigint \
               FROM sarana",
            &[],
        )
        .await
        .context("ringkas_sarana")?;
    Ok((r.get(0), r.get(1), r.get(2)))
}

pub async fn insert_sarana(
    pool: &Pool,
    nama: &str,
    kategori: &str,
    lokasi: &str,
    jumlah: i32,
    kondisi: &str,
    catatan: &str,
    oleh: i64,
) -> Result<i64> {
    let c = pool.get().await?;
    let r = c
        .query_one(
            "INSERT INTO sarana (nama, kategori, lokasi, jumlah, kondisi, catatan, dicatat_oleh) \
             VALUES ($1,$2,$3,$4,$5,$6,$7) RETURNING id",
            &[&nama, &kategori, &lokasi, &jumlah, &kondisi, &catatan, &oleh],
        )
        .await
        .context("insert_sarana")?;
    Ok(r.get(0))
}

/// Ubah satu baris. `false` = barisnya tak ada (sudah dihapus orang lain
/// sementara layarnya masih terbuka).
pub async fn update_sarana(
    pool: &Pool,
    id: i64,
    nama: &str,
    kategori: &str,
    lokasi: &str,
    jumlah: i32,
    kondisi: &str,
    catatan: &str,
    oleh: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE sarana SET nama=$2, kategori=$3, lokasi=$4, jumlah=$5, kondisi=$6, \
                    catatan=$7, dicatat_oleh=$8, updated_at=NOW() \
              WHERE id=$1",
            &[&id, &nama, &kategori, &lokasi, &jumlah, &kondisi, &catatan, &oleh],
        )
        .await
        .context("update_sarana")?;
    Ok(n > 0)
}

pub async fn delete_sarana(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM sarana WHERE id = $1", &[&id])
        .await
        .context("delete_sarana")?;
    Ok(n > 0)
}
