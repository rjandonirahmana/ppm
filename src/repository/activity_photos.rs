//! repository/activity_photos.rs — Query galeri foto kegiatan (migrasi 34).
//! Ditampilkan publik di beranda; dikelola staf di /galeri (urutan bisa di-drag).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

use crate::models::ActivityPhoto;

/// Semua foto terurut `sort_order` lalu `id` (tie-breaker stabil).
pub async fn list_activity_photos(pool: &Pool) -> Result<Vec<ActivityPhoto>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, url, caption, sort_order, focus_x, focus_y, zoom, fit \
             FROM activity_photos ORDER BY sort_order, id",
            &[],
        )
        .await
        .context("list_activity_photos")?;
    Ok(rows
        .into_iter()
        .map(|r| ActivityPhoto {
            id: r.get(0),
            url: r.get(1),
            caption: r.get(2),
            sort_order: r.get(3),
            focus_x: r.get(4),
            focus_y: r.get(5),
            zoom: r.get(6),
            fit: r.get(7),
        })
        .collect())
}

/// Tambah foto baru di urutan paling belakang (max+1).
/// Bidikan foto: titik fokus, perbesaran, dan mode isi bingkai.
/// Dilewatkan utuh dari editor supaya foto tersimpan LANGSUNG dengan bidikan
/// yang barusan diatur — bukan tersimpan di tengah lalu diperbaiki menyusul.
#[derive(Clone, Copy, Debug)]
pub struct PhotoFraming {
    pub focus_x: f32,
    pub focus_y: f32,
    pub zoom: f32,
    pub fit: &'static str,
}

impl Default for PhotoFraming {
    fn default() -> Self {
        let (focus_x, focus_y, zoom) = crate::models::FOCUS_DEFAULT;
        Self { focus_x, focus_y, zoom, fit: "cover" }
    }
}

pub async fn insert_activity_photo(
    pool: &Pool,
    url: &str,
    caption: &str,
    created_by: i64,
    f: PhotoFraming,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO activity_photos \
                 (url, caption, sort_order, created_by, focus_x, focus_y, zoom, fit) \
             VALUES ($1, $2, \
                     (SELECT COALESCE(MAX(sort_order), 0) + 1 FROM activity_photos), \
                     $3, $4, $5, $6, $7) \
             RETURNING id",
            &[&url, &caption, &created_by, &f.focus_x, &f.focus_y, &f.zoom, &f.fit],
        )
        .await
        .context("insert_activity_photo")?;
    Ok(row.get(0))
}

pub async fn delete_activity_photo(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM activity_photos WHERE id = $1", &[&id])
        .await
        .context("delete_activity_photo")?;
    Ok(n > 0)
}

/// Set ulang `sort_order` sesuai posisi id dalam `ids` (hasil drag-reorder).
/// Id yang tak ada di tabel diabaikan (join). Satu query — aman & atomik.
pub async fn reorder_activity_photos(pool: &Pool, ids: &[i64]) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE activity_photos a \
         SET sort_order = t.ord::int \
         FROM unnest($1::bigint[]) WITH ORDINALITY AS t(id, ord) \
         WHERE a.id = t.id",
        &[&ids],
    )
    .await
    .context("reorder_activity_photos")?;
    Ok(())
}

/// Simpan titik fokus & perbesaran satu foto (migrasi 54).
///
/// Pemanggil WAJIB sudah merapikan nilainya lewat `models::clamp_focus`; CHECK
/// di tabel hanya jaring pengaman terakhir, dan menabraknya berarti request
/// gagal dengan galat Postgres, bukan pesan yang berguna bagi pengguna.
pub async fn set_activity_photo_focus(
    pool: &Pool,
    id: i64,
    f: PhotoFraming,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE activity_photos \
             SET focus_x = $2, focus_y = $3, zoom = $4, fit = $5 WHERE id = $1",
            &[&id, &f.focus_x, &f.focus_y, &f.zoom, &f.fit],
        )
        .await
        .context("set_activity_photo_focus")?;
    Ok(n > 0)
}
