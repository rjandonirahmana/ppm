//! service/hafalan.rs — Catat & baca setoran hafalan (kerangka laporan
//! akademik kategori "Mengaji").

use anyhow::Result;
use deadpool_postgres::Pool;

use super::fmt::{fmt_date, wib};
use crate::models::{HafalanItem, SessionUser};
use crate::repository as repo;

fn is_staff(role: &str) -> bool {
    matches!(role, "admin" | "supervisor" | "dewan_guru" | "teacher")
}

/// Catat satu setoran hafalan santri — staf saja.
#[allow(clippy::too_many_arguments)]
pub async fn log_hafalan(
    pool: &Pool,
    staff: &SessionUser,
    student_id: i64,
    class_id: Option<i64>,
    surah: &str,
    ayat_range: &str,
    juz: Option<i16>,
    quality: &str,
    note: &str,
) -> Result<()> {
    if !is_staff(&staff.role) {
        anyhow::bail!("forbidden");
    }
    let surah = surah.trim();
    if surah.is_empty() {
        anyhow::bail!("Nama surah wajib diisi.");
    }
    let quality = match quality {
        "perlu_perbaikan" | "mengulang" => quality,
        _ => "lancar",
    };
    repo::insert_hafalan(
        pool, student_id, class_id, staff.id, surah, ayat_range.trim(), juz, quality, note.trim(),
    )
    .await?;
    tracing::info!(by = staff.id, student_id, surah, "setoran hafalan dicatat");
    Ok(())
}

fn to_items(rows: Vec<repo::HafalanRow>) -> Vec<HafalanItem> {
    let wib_tz = wib();
    rows.into_iter()
        .map(|r| crate::models::HafalanItem {
            id: r.id,
            surah: r.surah,
            ayat_range: r.ayat_range,
            juz: r.juz,
            quality_label: crate::models::quality_label(&r.quality).into(),
            quality: r.quality,
            note: r.note,
            date_label: fmt_date(r.created_at.with_timezone(&wib_tz).date_naive()),
            recorded_by: r.recorded_by_name.unwrap_or_else(|| "-".into()),
        })
        .collect()
}

/// Riwayat setoran seorang santri — dipakai rapor pribadi & laporan ortu.
pub async fn hafalan_history(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<HafalanItem>> {
    Ok(to_items(repo::recent_hafalan(pool, user_id, limit).await?))
}

/// Setoran terbaru satu kelas (panel Setoran Hafalan di detail sesi).
pub async fn hafalan_of_class(pool: &Pool, class_id: i64, limit: i64) -> Result<Vec<(String, HafalanItem)>> {
    let rows = repo::recent_hafalan_of_class(pool, class_id, limit).await?;
    let wib_tz = wib();
    Ok(rows
        .into_iter()
        .map(|(name, r)| {
            (
                name,
                HafalanItem {
                    id: r.id,
                    surah: r.surah,
                    ayat_range: r.ayat_range,
                    juz: r.juz,
                    quality_label: crate::models::quality_label(&r.quality).into(),
                    quality: r.quality,
                    note: r.note,
                    date_label: fmt_date(r.created_at.with_timezone(&wib_tz).date_naive()),
                    recorded_by: r.recorded_by_name.unwrap_or_else(|| "-".into()),
                },
            )
        })
        .collect())
}
