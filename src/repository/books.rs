//! repository/books.rs — Query tabel books + academic_user (migrasi 18).
//!
//! `missing_pages` disimpan JSONB array pasangan [awal,akhir] — query-friendly
//! (jsonb_array_length, dst) tanpa parsing string di aplikasi. Dikonversi
//! ke/dari teks "11-20, 45-50" di layer service (lihat service/books.rs) agar
//! form input tetap satu kotak teks.

use anyhow::{Context, Result};
use deadpool_postgres::Pool;
use serde_json::Value;

pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub total_pages: i32,
}

pub async fn list_books(pool: &Pool) -> Result<Vec<BookRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, title, total_pages FROM books WHERE deleted_at IS NULL ORDER BY title",
            &[],
        )
        .await
        .context("list_books")?;
    Ok(rows
        .into_iter()
        .map(|r| BookRow { id: r.get(0), title: r.get(1), total_pages: r.get(2) })
        .collect())
}

pub async fn get_book(pool: &Pool, id: i64) -> Result<Option<BookRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, title, total_pages FROM books WHERE id = $1 AND deleted_at IS NULL",
            &[&id],
        )
        .await
        .context("get_book")?;
    Ok(row.map(|r| BookRow { id: r.get(0), title: r.get(1), total_pages: r.get(2) }))
}

pub async fn create_book(pool: &Pool, title: &str, total_pages: i32) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO books (title, total_pages) VALUES ($1, $2) RETURNING id",
            &[&title, &total_pages],
        )
        .await
        .context("create_book")?;
    Ok(row.get(0))
}

/// Soft delete (deleted_at). Baris academic_user terkait dibiarkan (histori),
/// tapi buku tak lagi tampil di daftar aktif.
pub async fn delete_book(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE books SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
            &[&id],
        )
        .await
        .context("delete_book")?;
    Ok(n > 0)
}

pub struct ProgressRow {
    pub book_id: i64,
    pub book_title: String,
    pub total_pages: i32,
    pub percentage: i16,
    pub missing_pages: Value,
}

/// Progres santri di SEMUA buku aktif (LEFT JOIN academic_user — buku tanpa
/// baris academic_user tampil dgn percentage 0 & missing_pages kosong).
pub async fn student_book_progress(pool: &Pool, user_id: i64) -> Result<Vec<ProgressRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT b.id, b.title, b.total_pages, \
                    COALESCE(a.percentage, 0::smallint), COALESCE(a.missing_pages, '[]'::jsonb) \
             FROM books b \
             LEFT JOIN academic_user a ON a.book_id = b.id AND a.user_id = $1 \
             WHERE b.deleted_at IS NULL ORDER BY b.title",
            &[&user_id],
        )
        .await
        .context("student_book_progress")?;
    Ok(rows
        .into_iter()
        .map(|r| ProgressRow {
            book_id: r.get(0),
            book_title: r.get(1),
            total_pages: r.get(2),
            percentage: r.get(3),
            missing_pages: r.get(4),
        })
        .collect())
}

pub struct StudentAcademicRow {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    /// Rata-rata persentase LINTAS SEMUA buku aktif (buku tanpa academic_user
    /// dihitung 0%, bukan diabaikan) — audit "seberapa jauh" per santri.
    pub avg_percentage: i32,
    pub books_started: i64,
    pub total_books: i64,
}

/// Ringkasan akademik SEMUA santri aktif, terurut PALING TERTINGGAL dulu
/// (avg_percentage ASC) — audit "Ihi" halaman hadist/quran.
pub async fn all_students_academic_summary(pool: &Pool) -> Result<Vec<StudentAcademicRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, \
                    COALESCE(ROUND(AVG(COALESCE(a.percentage, 0))), 0)::INT, \
                    COUNT(a.id) FILTER (WHERE a.percentage > 0), \
                    (SELECT COUNT(*) FROM books WHERE deleted_at IS NULL) \
             FROM users u \
             CROSS JOIN books b \
             LEFT JOIN academic_user a ON a.user_id = u.id AND a.book_id = b.id \
             WHERE u.role = 'santri' AND u.is_active = TRUE AND b.deleted_at IS NULL \
             GROUP BY u.id, u.full_name, u.nis \
             ORDER BY 4 ASC, u.full_name",
            &[],
        )
        .await
        .context("all_students_academic_summary")?;
    Ok(rows
        .into_iter()
        .map(|r| StudentAcademicRow {
            user_id: r.get(0),
            name: r.get(1),
            nis: r.get(2),
            avg_percentage: r.get(3),
            books_started: r.get(4),
            total_books: r.get(5),
        })
        .collect())
}

/// Simpan/ubah progres (upsert) — satu baris per (user_id, book_id).
pub async fn upsert_progress(
    pool: &Pool,
    user_id: i64,
    book_id: i64,
    percentage: i16,
    missing_pages: &Value,
    updated_by: i64,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO academic_user (user_id, book_id, percentage, missing_pages, updated_by) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id, book_id) DO UPDATE SET \
            percentage = EXCLUDED.percentage, missing_pages = EXCLUDED.missing_pages, \
            updated_by = EXCLUDED.updated_by, updated_at = NOW()",
        &[&user_id, &book_id, &percentage, missing_pages, &updated_by],
    )
    .await
    .context("upsert_progress")?;
    Ok(())
}
