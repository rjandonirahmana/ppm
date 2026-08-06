//! service/semester.rs — Semester akademik (migrasi 40). Admin mendefinisikan
//! ganjil/genap + tahun + rentang tanggal; satu yang aktif jadi acuan
//! `current_semester` (kehadiran %, laporan).

use anyhow::Result;
use chrono::NaiveDate;
use deadpool_postgres::Pool;

use crate::models::SemesterItem;
use crate::repository as repo;

pub fn kind_label(kind: &str) -> &'static str {
    match kind {
        "ganjil" => "Ganjil",
        "genap" => "Genap",
        _ => "-",
    }
}

/// "Semester Ganjil 2026/2027" (tahun genap dimulai tahun+? tetap year/year+1
/// untuk konsistensi dgn label lama service::santri).
pub fn semester_label(kind: &str, year: i16) -> String {
    format!("Semester {} {}/{}", kind_label(kind), year, year + 1)
}

/// Hari ini (WIB) — acuan "semester sedang berjalan".
use super::fmt::today_wib;

fn to_item(r: repo::SemesterRow, today: NaiveDate) -> SemesterItem {
    // is_active = SEDANG BERJALAN (today di dalam rentang), otomatis dari tanggal.
    let is_current = r.start_date <= today && today <= r.end_date;
    SemesterItem {
        label: semester_label(&r.kind, r.year),
        kind_label: kind_label(&r.kind).to_string(),
        start_date: r.start_date.to_string(),
        end_date: r.end_date.to_string(),
        id: r.id,
        kind: r.kind,
        year: r.year,
        is_active: is_current,
    }
}

pub async fn list_semesters(pool: &Pool) -> Result<Vec<SemesterItem>> {
    let today = today_wib();
    Ok(repo::list_semesters(pool, 50).await?.into_iter().map(|r| to_item(r, today)).collect())
}

/// Label semester yang SEDANG BERJALAN menurut tanggal ("" bila hari ini di luar
/// semua rentang terdaftar). Otomatis — tanpa aktivasi manual.
pub async fn active_label(pool: &Pool) -> String {
    match repo::current_semester(pool, today_wib()).await {
        Ok(Some(s)) => semester_label(&s.kind, s.year),
        _ => String::new(),
    }
}

/// Periksa masukan form semester dan ubah jadi nilai yang siap disimpan.
///
/// SATU tempat untuk aturan yang berlaku sama bagi pembuatan maupun penyuntingan
/// — kalau tidak, "tanggal tak boleh mundur" dan batas tahun harus ditulis dua
/// kali dan cepat atau lambat yang satu ikut berubah sementara yang lain tidak.
///
/// `exclude_id` = baris yang TIDAK dihitung sebagai tabrakan: 0 saat membuat
/// baru, dan id-nya sendiri saat menyunting — tanpa itu setiap semester selalu
/// bertabrakan dengan dirinya sendiri dan tak pernah bisa disimpan.
async fn periksa_masukan(
    pool: &Pool,
    kind: &str,
    year: &str,
    start: &str,
    end: &str,
    exclude_id: i64,
) -> Result<(i16, NaiveDate, NaiveDate)> {
    if !matches!(kind, "ganjil" | "genap") {
        bail_user!("Jenis semester harus ganjil atau genap.");
    }
    let year: i16 = year
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Tahun harus angka (mis. 2026)."))?;
    if !(1990..=2100).contains(&year) {
        bail_user!("Tahun tidak masuk akal (1990–2100).");
    }
    let parse = |s: &str| NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d");
    let sd = parse(start).map_err(|_| anyhow::anyhow!("Tanggal mulai tidak valid (YYYY-MM-DD)."))?;
    let ed = parse(end).map_err(|_| anyhow::anyhow!("Tanggal selesai tidak valid (YYYY-MM-DD)."))?;
    // Tak boleh mundur: tanggal selesai harus SETELAH tanggal mulai.
    if ed <= sd {
        bail_user!("Tanggal selesai harus setelah tanggal mulai (tidak boleh mundur/sama).");
    }
    // Tak boleh tumpang tindih dengan semester lain (ujung yang sama pun ditolak —
    // mis. ganjil s/d 1 Agu → genap harus mulai ≥ 2 Agu).
    if let Some(bentrok) = repo::overlapping_semester(pool, sd, ed, exclude_id).await? {
        bail_user!(
            "Rentang tanggal bertabrakan dengan {} ({} → {}). Mulai minimal sehari setelahnya.",
            semester_label(&bentrok.kind, bentrok.year),
            bentrok.start_date,
            bentrok.end_date
        );
    }
    Ok((year, sd, ed))
}

/// Buat semester baru (admin). `year`/tanggal dari teks form. Menolak rentang
/// terbalik (mundur) & yang tumpang tindih dgn semester lain (ujung sama = tabrakan).
pub async fn create_semester(
    pool: &Pool,
    kind: &str,
    year: &str,
    start: &str,
    end: &str,
) -> Result<i64> {
    let (year, sd, ed) = periksa_masukan(pool, kind, year, start, end, 0).await?;
    repo::create_semester(pool, kind, year, sd, ed).await
}

/// Sunting semester yang sudah ada. Aturannya sama persis dengan pembuatan —
/// bedanya hanya baris ini sendiri tidak dihitung sebagai tabrakan.
///
/// Status "SEDANG BERJALAN" tak ikut disunting: ia dihitung dari tanggal hari
/// ini (`to_item`), jadi mengubah rentangnya sudah otomatis memindahkannya.
pub async fn update_semester(
    pool: &Pool,
    id: i64,
    kind: &str,
    year: &str,
    start: &str,
    end: &str,
) -> Result<()> {
    let (year, sd, ed) = periksa_masukan(pool, kind, year, start, end, id).await?;
    if !repo::update_semester(pool, id, kind, year, sd, ed).await? {
        bail_user!("Semester tidak ditemukan.");
    }
    Ok(())
}

pub async fn delete_semester(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_semester(pool, id).await? {
        bail_user!("Semester tidak ditemukan.");
    }
    Ok(())
}
