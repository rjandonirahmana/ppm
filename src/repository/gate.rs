//! repository/gate.rs — RFID gerbang UTAMA pondok (masuk/keluar), terpisah
//! dari gerbang kelas (repository/attendance.rs::insert_attendance).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

/// Jendela abai tap gerbang berturut-turut. 10 detik: cukup lama menelan
/// pantulan pembaca kartu, cukup pendek sehingga orang yang benar-benar keluar
/// lalu masuk lagi (mis. lupa barang) tak terhalang.
const GATE_DEBOUNCE_SECS: i64 = 10;

/// Toggle status gerbang satu user: baca status terkini → balik arah → catat
/// log + update cache `users.gate_status/gate_at` — SATU transaksi (baca+tulis
/// harus konsisten; dua request nyaris bersamaan tak boleh saling menimpa).
/// Return arah BARU ("in"/"out").
pub async fn toggle_gate(pool: &Pool, user_id: i64, device_id: Option<i64>) -> Result<String> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("toggle_gate: begin")?;

    let row = tx
        .query_one(
            "SELECT gate_status, gate_at FROM users WHERE id = $1 FOR UPDATE",
            &[&user_id],
        )
        .await
        .context("toggle_gate: select")?;
    let cur: String = row.get(0);
    let last: Option<chrono::DateTime<chrono::Utc>> = row.get(1);

    // DEBOUNCE. Kartu yang memantul di pembaca, atau ditahan sebentar, mengirim
    // dua tap dalam hitungan detik. Tanpa jendela abai ini: keluar lalu masuk
    // lagi seketika → status akhir SALAH dan riwayatnya berisi dua baris palsu.
    //
    // Tap dalam jendela dianggap tap yang SAMA: kembalikan status sekarang
    // tanpa membalik apa pun (idempoten), tanpa menulis log.
    if let Some(t) = last {
        if (chrono::Utc::now() - t) < chrono::Duration::seconds(GATE_DEBOUNCE_SECS) {
            tx.rollback().await.ok();
            tracing::debug!(user_id, "tap gerbang diabaikan (debounce)");
            return Ok(cur);
        }
    }

    let next = if cur == "out" { "in" } else { "out" };

    tx.execute(
        "UPDATE users SET gate_status = $2, gate_at = NOW() WHERE id = $1",
        &[&user_id, &next],
    )
    .await
    .context("toggle_gate: update users")?;
    tx.execute(
        "INSERT INTO gate_logs (user_id, device_id, direction) VALUES ($1, $2, $3)",
        &[&user_id, &device_id, &next],
    )
    .await
    .context("toggle_gate: insert log")?;

    tx.commit().await.context("toggle_gate: commit")?;
    Ok(next.to_string())
}

pub struct OutsideRow {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub gate_at: Option<DateTime<Utc>>,
}

/// Santri yang statusnya SEDANG "di luar pondok" (laporan admin/pamong).
pub async fn students_outside(pool: &Pool, limit: i64) -> Result<Vec<OutsideRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, c.name, u.gate_at \
             FROM users u \
             LEFT JOIN classes c ON c.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE AND u.gate_status = 'out' \
             ORDER BY u.gate_at DESC NULLS LAST LIMIT $1",
            &[&limit],
        )
        .await
        .context("students_outside")?;
    Ok(rows
        .into_iter()
        .map(|r| OutsideRow {
            user_id: r.get(0),
            name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            gate_at: r.get(4),
        })
        .collect())
}

pub async fn count_outside(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FROM users WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE AND gate_status = 'out'",
            &[],
        )
        .await
        .context("count_outside")?;
    Ok(row.get(0))
}

/// Status gerbang + kapan terakhir berubah, utk satu santri (rapor pribadi /
/// laporan ortu).
pub async fn gate_status_of(pool: &Pool, user_id: i64) -> Result<(String, Option<DateTime<Utc>>)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT gate_status, gate_at FROM users WHERE id = $1",
            &[&user_id],
        )
        .await
        .context("gate_status_of")?;
    Ok((row.get(0), row.get(1)))
}
