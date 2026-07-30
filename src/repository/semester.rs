//! repository/semester.rs — Semester akademik yang didefinisikan admin
//! (migrasi 40). Maksimal SATU aktif (partial unique index di DB).

use anyhow::{Context, Result};
use chrono::NaiveDate;
use deadpool_postgres::Pool;

pub struct SemesterRow {
    pub id: i64,
    pub kind: String,
    pub year: i16,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub is_active: bool,
}

fn row(r: tokio_postgres::Row) -> SemesterRow {
    SemesterRow {
        id: r.get(0),
        kind: r.get(1),
        year: r.get(2),
        start_date: r.get(3),
        end_date: r.get(4),
        is_active: r.get(5),
    }
}

const COLS: &str = "id, kind, year, start_date, end_date, is_active";

/// Buat semester baru (tak langsung aktif). Return id.
pub async fn create_semester(
    pool: &Pool,
    kind: &str,
    year: i16,
    start: NaiveDate,
    end: NaiveDate,
) -> Result<i64> {
    let c = pool.get().await?;
    let r = c
        .query_one(
            "INSERT INTO academic_semesters (kind, year, start_date, end_date) \
             VALUES ($1, $2, $3, $4) RETURNING id",
            &[&kind, &year, &start, &end],
        )
        .await
        .context("create_semester")?;
    Ok(r.get(0))
}

pub async fn list_semesters(pool: &Pool, limit: i64) -> Result<Vec<SemesterRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT {COLS} FROM academic_semesters ORDER BY year DESC, kind DESC, id DESC LIMIT $1"
    );
    let rows = c.query(&sql, &[&limit]).await.context("list_semesters")?;
    Ok(rows.into_iter().map(row).collect())
}

/// Semester yang SEDANG BERJALAN menurut tanggal (today ∈ [start, end]).
/// Otomatis — tak perlu diaktifkan manual. None bila hari ini di luar semua
/// rentang yang terdaftar.
pub async fn current_semester(pool: &Pool, today: NaiveDate) -> Result<Option<SemesterRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT {COLS} FROM academic_semesters \
         WHERE $1 BETWEEN start_date AND end_date ORDER BY start_date DESC LIMIT 1"
    );
    let r = c.query_opt(&sql, &[&today]).await.context("current_semester")?;
    Ok(r.map(row))
}

/// Semester terdaftar yang rentang tanggalnya BERSINGGUNGAN dengan [start, end]
/// (inklusif — ujung yang sama pun dianggap tumpang tindih). None = aman.
/// `exclude_id` mengecualikan baris tertentu (untuk edit; 0 = tak ada).
pub async fn overlapping_semester(
    pool: &Pool,
    start: NaiveDate,
    end: NaiveDate,
    exclude_id: i64,
) -> Result<Option<SemesterRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "SELECT {COLS} FROM academic_semesters \
         WHERE id <> $3 AND start_date <= $2 AND end_date >= $1 \
         ORDER BY start_date LIMIT 1"
    );
    let r = c
        .query_opt(&sql, &[&start, &end, &exclude_id])
        .await
        .context("overlapping_semester")?;
    Ok(r.map(row))
}

pub async fn delete_semester(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM academic_semesters WHERE id = $1", &[&id])
        .await
        .context("delete_semester")?;
    Ok(n > 0)
}
