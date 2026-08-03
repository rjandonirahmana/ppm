//! service/books.rs — Materi (Qur'an/Hadist, migrasi 18/25): kelola daftar
//! materi + progres per santri. Quran = daftar surat (nama+ayat), unit = ayat;
//! Hadist = jumlah halaman, unit = halaman. Progres = peta unit→status (1
//! setengah / 2 penuh) yang diisi santri via grid; percentage dihitung ulang.
//!
//! CATATAN: `parse_page_ranges`/`format_page_ranges` DIPERTAHANKAN — dipakai
//! service/kelas.rs & service/sessions.rs utk materi buku PER SESI (migrasi 20),
//! bukan bagian sistem unit_status ini.

use std::collections::HashMap;

use anyhow::Result;
use deadpool_postgres::Pool;
use serde_json::Value;

use crate::models::{BookItem, BookProgressItem, StudentAcademicItem, Surah};
use crate::repository as repo;

fn value_to_surahs(v: &Value) -> Vec<Surah> {
    v.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|s| {
                    let name = s.get("name")?.as_str()?.to_string();
                    let ayat = s.get("ayat")?.as_i64()? as i32;
                    Some(Surah { name, ayat })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn value_to_unit_status(v: &Value) -> HashMap<String, u8> {
    v.as_object()
        .map(|o| {
            o.iter()
                .filter_map(|(k, val)| {
                    let s = val.as_i64()? as u8;
                    (s == 1 || s == 2).then(|| (k.clone(), s))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Persentase dari peta unit_status: SUM(nilai) / (total*2) * 100. Penuh semua
/// = 100%. total<=0 → 0.
fn compute_percentage(unit_status: &Value, total: i32) -> i16 {
    if total <= 0 {
        return 0;
    }
    let sum: i64 = unit_status
        .as_object()
        .map(|o| o.values().filter_map(|v| v.as_i64()).filter(|n| *n == 1 || *n == 2).sum())
        .unwrap_or(0);
    ((sum as f64) / (total as f64 * 2.0) * 100.0).round().clamp(0.0, 100.0) as i16
}

pub async fn list_books(pool: &Pool) -> Result<Vec<BookItem>> {
    Ok(repo::list_books(pool)
        .await?
        .into_iter()
        .map(|b| BookItem {
            id: b.id,
            title: b.title,
            category: b.category,
            total_pages: b.total_pages,
            surahs: value_to_surahs(&b.surahs),
        })
        .collect())
}

/// Validasi + normalisasi input materi → (title, category, total_pages, surahs
/// JSONB). Dipakai create & update. quran: surahs_json wajib ≥1, total = Σ ayat.
/// hadist: pages > 0, surahs kosong.
fn parse_book_input(
    title: &str,
    category: &str,
    pages: &str,
    surahs_json: &str,
) -> Result<(String, String, i32, Value)> {
    let title = title.trim();
    if title.is_empty() {
        bail_user!("Judul materi wajib diisi.");
    }
    let category = if category == "quran" { "quran" } else { "hadist" };
    if category == "quran" {
        let parsed: Vec<Surah> = serde_json::from_str(surahs_json.trim())
            .map_err(|_| anyhow::anyhow!("Daftar surat tidak valid."))?;
        let surahs: Vec<Surah> = parsed
            .into_iter()
            .filter(|s| !s.name.trim().is_empty() && s.ayat > 0)
            .collect();
        if surahs.is_empty() {
            bail_user!("Tambahkan minimal satu surat (nama + jumlah ayat).");
        }
        let total: i32 = surahs.iter().map(|s| s.ayat).sum();
        Ok((title.into(), "quran".into(), total, serde_json::to_value(&surahs)?))
    } else {
        let pages: i32 = pages
            .trim()
            .parse()
            .map_err(|_| anyhow::anyhow!("Jumlah halaman harus berupa angka."))?;
        if pages <= 0 {
            bail_user!("Jumlah halaman harus lebih dari 0.");
        }
        Ok((title.into(), "hadist".into(), pages, Value::Array(vec![])))
    }
}

/// Buat materi. `category` = "quran" | "hadist".
pub async fn create_book(
    pool: &Pool,
    title: &str,
    category: &str,
    pages: &str,
    surahs_json: &str,
) -> Result<i64> {
    let (title, cat, total, surahs) = parse_book_input(title, category, pages, surahs_json)?;
    repo::create_book(pool, &title, &cat, total, &surahs).await
}

/// Ubah materi. CATATAN: mengubah struktur unit (kategori/halaman/surat) bisa
/// membuat progres santri lama tak lagi sinkron — keputusan admin.
pub async fn update_book(
    pool: &Pool,
    id: i64,
    title: &str,
    category: &str,
    pages: &str,
    surahs_json: &str,
) -> Result<()> {
    let (title, cat, total, surahs) = parse_book_input(title, category, pages, surahs_json)?;
    if !repo::update_book(pool, id, &title, &cat, total, &surahs).await? {
        bail_user!("Materi tidak ditemukan.");
    }
    Ok(())
}

pub async fn delete_book(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_book(pool, id).await? {
        bail_user!("Materi tidak ditemukan.");
    }
    Ok(())
}

pub async fn student_progress(pool: &Pool, user_id: i64) -> Result<Vec<BookProgressItem>> {
    Ok(repo::student_book_progress(pool, user_id)
        .await?
        .into_iter()
        .map(|r| BookProgressItem {
            book_id: r.book_id,
            book_title: r.book_title,
            category: r.category,
            total_pages: r.total_pages,
            surahs: value_to_surahs(&r.surahs),
            unit_status: value_to_unit_status(&r.unit_status),
            percentage: r.percentage,
        })
        .collect())
}

/// Progres materi satu santri DARI SUDUT PANDANG pengguna lain (orang tua /
/// staf / guru). Authorization: admin/dewan/guru/pamong bebas; orang tua hanya
/// anak yang terhubung; santri hanya dirinya sendiri.
pub async fn student_progress_for_viewer(
    pool: &Pool,
    viewer_id: i64,
    viewer_role: &str,
    student_id: i64,
) -> Result<Vec<BookProgressItem>> {
    let allowed = match viewer_role {
        "admin" | "ketua" | "dewan_guru" | "supervisor" | "teacher" => true,
        "parent" => repo::is_connected(pool, viewer_id, student_id).await?,
        "santri" | "santri_finance" => viewer_id == student_id,
        _ => false,
    };
    if !allowed {
        bail_user!("forbidden");
    }
    student_progress(pool, student_id).await
}

/// Audit akademik SEMUA santri — rata-rata persentase lintas materi.
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

/// Simpan progres santri: `unit_status_json` = JSON `{"<unit>": 1|2}` (dari grid
/// klien). percentage dihitung ulang dari total unit materi.
pub async fn set_unit_status(
    pool: &Pool,
    actor_id: i64,
    user_id: i64,
    book_id: i64,
    unit_status_json: &str,
) -> Result<()> {
    let Some(book) = repo::get_book(pool, book_id).await? else {
        bail_user!("Materi tidak ditemukan.");
    };
    let raw: Value = serde_json::from_str(unit_status_json.trim().is_empty().then_some("{}").unwrap_or(unit_status_json.trim()))
        .map_err(|_| anyhow::anyhow!("Data progres tidak valid."))?;
    // Bersihkan: hanya nilai 1/2 (kosong tak disimpan).
    let clean: serde_json::Map<String, Value> = raw
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(_, v)| matches!(v.as_i64(), Some(1) | Some(2)))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let unit_status = Value::Object(clean);
    let pct = compute_percentage(&unit_status, book.total_pages);
    repo::upsert_progress(pool, user_id, book_id, pct, &unit_status, actor_id).await
}

// ── Rentang halaman materi PER SESI (migrasi 20) — beda dari unit_status ─────
// Dipakai service/kelas.rs (create/set_session_book) & service/sessions.rs.

/// Parse "11-20, 45-50, 23" → JSONB array [[11,20],[45,50],[23,23]].
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
            bail_user!("Rentang halaman \"{part}\" di luar batas (materi ini {total_pages} halaman/ayat).");
        }
        ranges.push(serde_json::json!([start, end]));
    }
    Ok(Value::Array(ranges))
}

/// Format JSONB array → "11-20, 45-50, 23".
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
