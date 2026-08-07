//! repository/books.rs — Query tabel books + academic_user (migrasi 18/25).
//! books.category ('quran'|'hadist'), books.surahs (JSONB [{name,ayat}] utk quran),
//! academic_user.unit_status (JSONB peta unit→status 1/2). Konversi Value↔tipe
//! domain di layer service.

use anyhow::{Context, Result};
use deadpool_postgres::Pool;
use serde_json::Value;

pub struct BookRow {
    pub id: i64,
    pub title: String,
    pub category: String,
    pub total_pages: i32,
    pub surahs: Value,
}

const BOOK_COLS: &str = "id, title, category, total_pages, surahs";

pub async fn list_books(pool: &Pool) -> Result<Vec<BookRow>> {
    let c = pool.get().await?;
    let sql = format!("SELECT {BOOK_COLS} FROM books WHERE deleted_at IS NULL ORDER BY title");
    let rows = c.query(&sql, &[]).await.context("list_books")?;
    Ok(rows.into_iter().map(row_to_book).collect())
}

pub async fn get_book(pool: &Pool, id: i64) -> Result<Option<BookRow>> {
    let c = pool.get().await?;
    let sql = format!("SELECT {BOOK_COLS} FROM books WHERE id = $1 AND deleted_at IS NULL");
    let row = c.query_opt(&sql, &[&id]).await.context("get_book")?;
    Ok(row.map(row_to_book))
}

fn row_to_book(r: tokio_postgres::Row) -> BookRow {
    BookRow {
        id: r.get(0),
        title: r.get(1),
        category: r.get(2),
        total_pages: r.get(3),
        surahs: r.get(4),
    }
}

pub async fn create_book(
    pool: &Pool,
    title: &str,
    category: &str,
    total_pages: i32,
    surahs: &Value,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO books (title, category, total_pages, surahs) \
             VALUES ($1, $2, $3, $4) RETURNING id",
            &[&title, &category, &total_pages, surahs],
        )
        .await
        .context("create_book")?;
    Ok(row.get(0))
}

pub async fn update_book(
    pool: &Pool,
    id: i64,
    title: &str,
    category: &str,
    total_pages: i32,
    surahs: &Value,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE books SET title = $2, category = $3, total_pages = $4, surahs = $5 \
             WHERE id = $1 AND deleted_at IS NULL",
            &[&id, &title, &category, &total_pages, surahs],
        )
        .await
        .context("update_book")?;
    Ok(n > 0)
}

/// Soft delete (deleted_at). Baris academic_user terkait dibiarkan (histori).
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
    pub category: String,
    pub total_pages: i32,
    pub surahs: Value,
    pub percentage: i16,
    pub unit_status: Value,
}

/// Progres santri di SEMUA materi aktif (LEFT JOIN academic_user — materi tanpa
/// baris academic_user tampil dgn percentage 0 & unit_status kosong).
pub async fn student_book_progress(pool: &Pool, user_id: i64) -> Result<Vec<ProgressRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT b.id, b.title, b.category, b.total_pages, b.surahs, \
                    COALESCE(a.percentage, 0::smallint), COALESCE(a.unit_status, '{}'::jsonb) \
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
            category: r.get(2),
            total_pages: r.get(3),
            surahs: r.get(4),
            percentage: r.get(5),
            unit_status: r.get(6),
        })
        .collect())
}

pub struct StudentAcademicRow {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    /// Rata-rata persentase LINTAS SEMUA materi aktif (materi tanpa academic_user
    /// dihitung 0%, bukan diabaikan) — audit "seberapa jauh" per santri.
    pub avg_percentage: i32,
    pub books_started: i64,
    pub total_books: i64,
}

/// Ringkasan akademik SEMUA santri aktif, terurut PALING TERTINGGAL dulu.
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
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE AND b.deleted_at IS NULL \
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
    unit_status: &Value,
    updated_by: i64,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO academic_user (user_id, book_id, percentage, unit_status, updated_by) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id, book_id) DO UPDATE SET \
            percentage = EXCLUDED.percentage, unit_status = EXCLUDED.unit_status, \
            updated_by = EXCLUDED.updated_by, updated_at = NOW()",
        &[&user_id, &book_id, &percentage, unit_status, &updated_by],
    )
    .await
    .context("upsert_progress")?;
    Ok(())
}

/// Hitung kekosongan per unit LANGSUNG DI SQL — hanya angkanya yang dikirim.
///
/// Versi pertama menarik seluruh `unit_status` tiap santri lalu menjumlahkan di
/// Rust. Benar, tapi boros: satu kelas 30 santri × Al Baqoroh berarti 30 objek
/// JSONB berisi ratusan kunci melintasi jaringan setiap kali panelnya dibuka —
/// puluhan sampai ratusan kilobyte untuk menghasilkan beberapa baris angka.
///
/// Di sini yang menyeberang hanya `(unit, kosong, setengah)`: satu baris per
/// ayat/halaman, berapa pun jumlah santrinya. Penggabungan jadi rentang tetap
/// di Rust — itu urusan penyajian, bukan penyimpanan.
///
/// Nilainya dibandingkan sebagai TEKS ('1'/'2'), bukan di-cast ke int: satu
/// nilai cacat di data lama akan menggagalkan SELURUH query bila memakai
/// `::int`, dan aturannya pun sama dengan `value_to_unit_status` — selain 1 & 2
/// dianggap belum tersentuh.
///
/// Return `(jumlah santri, [(kunci unit, kosong, setengah)])`.
pub async fn hitung_kekosongan(
    pool: &Pool,
    class_id: i64,
    book_id: i64,
    kunci: &[String],
) -> Result<(i64, Vec<(String, i64, i64)>)> {
    if kunci.is_empty() {
        return Ok((0, Vec::new()));
    }
    let c = pool.get().await?;
    let rows = c
        .query(
            "WITH st AS ( \
                SELECT COALESCE(a.unit_status, '{}'::jsonb) AS s \
                  FROM class_participants cp \
                  JOIN users u ON u.id = cp.user_id \
                       AND u.role IN ('santri', 'santri_finance') AND u.is_active \
                  LEFT JOIN academic_user a ON a.user_id = u.id AND a.book_id = $2 \
                 WHERE cp.class_id = $1 \
             ), \
             unit AS (SELECT k FROM unnest($3::text[]) AS k) \
             SELECT unit.k, \
                    count(*) FILTER ( \
                        WHERE st.s ->> unit.k IS NULL \
                           OR st.s ->> unit.k NOT IN ('1', '2'))::bigint AS kosong, \
                    count(*) FILTER (WHERE st.s ->> unit.k = '1')::bigint AS setengah, \
                    count(*)::bigint AS total \
               FROM unit CROSS JOIN st \
              GROUP BY unit.k",
            &[&class_id, &book_id, &kunci],
        )
        .await
        .context("hitung_kekosongan")?;

    // Kelas tanpa santri → CROSS JOIN kosong → tak ada baris sama sekali.
    // Itu keadaan sah, bukan galat: panel menyebutnya "belum ada santri".
    let total = rows.first().map(|r| r.get::<_, i64>(3)).unwrap_or(0);
    let mut per_unit: std::collections::HashMap<String, (i64, i64)> =
        std::collections::HashMap::with_capacity(rows.len());
    for r in &rows {
        per_unit.insert(r.get(0), (r.get(1), r.get(2)));
    }
    // Dikembalikan MENGIKUTI URUTAN kunci yang diminta — SQL tak menjamin
    // urutan GROUP BY, dan urutan itulah yang jadi urutan kitabnya.
    let hasil = kunci
        .iter()
        .map(|k| {
            let (kosong, setengah) = per_unit.get(k).copied().unwrap_or((total, 0));
            (k.clone(), kosong, setengah)
        })
        .collect();
    Ok((total, hasil))
}
