//! repository/hafalan.rs — Log setoran hafalan (kerangka laporan akademik
//! kategori "Mengaji"). Append-only, pola sama point_logs.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

#[allow(clippy::too_many_arguments)]
pub async fn insert_hafalan(
    pool: &Pool,
    user_id: i64,
    class_id: Option<i64>,
    recorded_by: i64,
    surah: &str,
    ayat_range: &str,
    juz: Option<i16>,
    quality: &str,
    note: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO hafalan_logs \
                (user_id, class_id, recorded_by, surah, ayat_range, juz, quality, note) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            &[&user_id, &class_id, &recorded_by, &surah, &ayat_range, &juz, &quality, &note],
        )
        .await
        .context("insert_hafalan")?;
    Ok(row.get(0))
}

pub struct HafalanRow {
    pub id: i64,
    pub surah: String,
    pub ayat_range: String,
    pub juz: Option<i16>,
    pub quality: String,
    pub note: String,
    pub created_at: DateTime<Utc>,
    pub recorded_by_name: Option<String>,
}

/// Riwayat setoran seorang santri, terbaru dulu.
pub async fn recent_hafalan(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<HafalanRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT h.id, h.surah, h.ayat_range, h.juz, h.quality, COALESCE(h.note, ''), \
                    h.created_at, r.full_name \
             FROM hafalan_logs h LEFT JOIN users r ON r.id = h.recorded_by \
             WHERE h.user_id = $1 ORDER BY h.created_at DESC LIMIT $2",
            &[&user_id, &limit],
        )
        .await
        .context("recent_hafalan")?;
    Ok(rows
        .into_iter()
        .map(|r| HafalanRow {
            id: r.get(0),
            surah: r.get(1),
            ayat_range: r.get(2),
            juz: r.get(3),
            quality: r.get(4),
            note: r.get(5),
            created_at: r.get(6),
            recorded_by_name: r.get(7),
        })
        .collect())
}

/// Setoran terbaru milik SATU kelas (panel Setoran Hafalan di detail sesi).
pub async fn recent_hafalan_of_class(pool: &Pool, class_id: i64, limit: i64) -> Result<Vec<(String, HafalanRow)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT h.id, h.surah, h.ayat_range, h.juz, h.quality, COALESCE(h.note, ''), \
                    h.created_at, r.full_name, u.full_name \
             FROM hafalan_logs h \
             JOIN users u ON u.id = h.user_id \
             LEFT JOIN users r ON r.id = h.recorded_by \
             WHERE h.class_id = $1 ORDER BY h.created_at DESC LIMIT $2",
            &[&class_id, &limit],
        )
        .await
        .context("recent_hafalan_of_class")?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let row = HafalanRow {
                id: r.get(0),
                surah: r.get(1),
                ayat_range: r.get(2),
                juz: r.get(3),
                quality: r.get(4),
                note: r.get(5),
                created_at: r.get(6),
                recorded_by_name: r.get(7),
            };
            (r.get(8), row)
        })
        .collect())
}

/// Jumlah juz "selesai" (distinct juz tercatat, bukan 'mengulang').
pub async fn juz_count(pool: &Pool, user_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(DISTINCT juz) FROM hafalan_logs \
             WHERE user_id = $1 AND juz IS NOT NULL AND quality <> 'mengulang'",
            &[&user_id],
        )
        .await
        .context("juz_count")?;
    Ok(row.get(0))
}

/// Ranking "Santri Teladan" (juz terbanyak, poin sbg tie-breaker) — laporan
/// kelas akademik dewan guru. `teacher_id` = None → seluruh pesantren.
pub async fn top_hafalan(pool: &Pool, limit: i64) -> Result<Vec<(i64, String, String, i64, i64)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, COALESCE(c.name, '-'), \
                    (SELECT COUNT(DISTINCT h.juz) FROM hafalan_logs h \
                        WHERE h.user_id = u.id AND h.juz IS NOT NULL AND h.quality <> 'mengulang') AS juz_n, \
                    u.points::BIGINT \
             FROM users u \
             LEFT JOIN classes c ON c.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE u.role = 'santri' AND u.is_active = TRUE \
             ORDER BY juz_n DESC, u.points DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("top_hafalan")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4))).collect())
}
