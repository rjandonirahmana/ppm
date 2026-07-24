//! service/books.rs — Buku materi hafalan (Qur'an/Hadist, migrasi 18): kelola
//! daftar buku (admin/pamong) + progres per santri (persentase + halaman
//! kosong). `missing_pages` disimpan JSONB di DB tapi diedit lewat SATU kotak
//! teks "11-20, 45-50" — parse/format terjadi di sini.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;
use serde_json::Value;

use crate::models::{BookItem, BookProgressItem, StudentAcademicItem};
use crate::repository as repo;

pub async fn list_books(pool: &Pool) -> Result<Vec<BookItem>> {
    Ok(repo::list_books(pool)
        .await?
        .into_iter()
        .map(|b| BookItem { id: b.id, title: b.title, total_pages: b.total_pages })
        .collect())
}

pub async fn create_book(pool: &Pool, title: &str, total_pages: &str) -> Result<i64> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Judul buku wajib diisi.");
    }
    let pages: i32 = total_pages
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Jumlah halaman harus berupa angka."))?;
    if pages <= 0 {
        bail!("Jumlah halaman harus lebih dari 0.");
    }
    repo::create_book(pool, title, pages).await
}

pub async fn delete_book(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_book(pool, id).await? {
        bail!("Buku tidak ditemukan.");
    }
    Ok(())
}

/// Parse "11-20, 45-50, 23" → JSONB array [[11,20],[45,50],[23,23]]. Validasi:
/// tiap halaman dalam rentang 1..=total_pages, awal <= akhir. Generik (bukan
/// cuma "halaman kosong") — dipakai juga service/kelas.rs utk materi buku
/// per sesi (migrasi 20), format JSONB SAMA dgn `academic_user.missing_pages`.
pub(crate) fn parse_page_ranges(text: &str, total_pages: i32) -> Result<Value> {
    let mut ranges = Vec::new();
    for part in text.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (start, end) = match part.split_once('-') {
            Some((a, b)) => {
                let a: i32 = a
                    .trim()
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Format rentang halaman tidak valid: \"{part}\""))?;
                let b: i32 = b
                    .trim()
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Format rentang halaman tidak valid: \"{part}\""))?;
                (a, b)
            }
            None => {
                let a: i32 = part
                    .parse()
                    .map_err(|_| anyhow::anyhow!("Format halaman tidak valid: \"{part}\""))?;
                (a, a)
            }
        };
        if start < 1 || end > total_pages || start > end {
            bail!("Rentang halaman \"{part}\" di luar batas (buku ini {total_pages} halaman).");
        }
        ranges.push(serde_json::json!([start, end]));
    }
    Ok(Value::Array(ranges))
}

/// Format JSONB array → "11-20, 45-50, 23" (angka tunggal bila awal==akhir).
pub(crate) fn format_page_ranges(v: &Value) -> String {
    let Some(arr) = v.as_array() else { return String::new() };
    arr.iter()
        .filter_map(|r| {
            let r = r.as_array()?;
            let a = r.first()?.as_i64()?;
            let b = r.get(1)?.as_i64()?;
            Some(if a == b { format!("{a}") } else { format!("{a}-{b}") })
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub async fn student_progress(pool: &Pool, user_id: i64) -> Result<Vec<BookProgressItem>> {
    Ok(repo::student_book_progress(pool, user_id)
        .await?
        .into_iter()
        .map(|r| BookProgressItem {
            book_id: r.book_id,
            book_title: r.book_title,
            total_pages: r.total_pages,
            percentage: r.percentage,
            missing_pages_label: format_page_ranges(&r.missing_pages),
        })
        .collect())
}

/// Audit akademik SEMUA santri (tab "Akademik" /students) — rata-rata
/// persentase lintas buku, paling tertinggal duluan.
pub async fn academic_audit(pool: &Pool) -> Result<Vec<StudentAcademicItem>> {
    Ok(repo::all_students_academic_summary(pool)
        .await?
        .into_iter()
        .map(|r| StudentAcademicItem {
            user_id: r.user_id,
            name: r.name,
            nis: r.nis.unwrap_or_else(|| "-".into()),
            avg_percentage: r.avg_percentage,
            books_started: r.books_started,
            total_books: r.total_books,
        })
        .collect())
}

pub async fn set_progress(
    pool: &Pool,
    actor_id: i64,
    user_id: i64,
    book_id: i64,
    percentage: &str,
    missing_pages_text: &str,
) -> Result<()> {
    let Some(book) = repo::get_book(pool, book_id).await? else {
        bail!("Buku tidak ditemukan.");
    };
    let pct: i16 = percentage
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Persentase harus berupa angka 0-100."))?;
    if !(0..=100).contains(&pct) {
        bail!("Persentase harus di antara 0 sampai 100.");
    }
    let missing = parse_page_ranges(missing_pages_text, book.total_pages)?;
    repo::upsert_progress(pool, user_id, book_id, pct, &missing, actor_id).await
}
