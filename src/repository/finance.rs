//! repository/finance.rs — Tagihan santri (migrasi 37).

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::models::BillItem;

/// Kolom + join standar → BillItem. `s` = santri, `v` = verifikator, `cl` = kelas utama.
const BILL_SELECT: &str = "SELECT b.id, b.user_id, s.full_name, COALESCE(s.nis,'-'), \
        COALESCE(cl.name,'-'), b.title, b.price, b.started_date, b.expired_date, \
        b.status, b.paid_at, b.paid_amount, COALESCE(b.method,''), COALESCE(b.proof_url,''), \
        COALESCE(v.full_name,''), b.note, \
        (b.status = 'belum' AND b.expired_date < CURRENT_DATE) AS overdue \
     FROM bills b \
     JOIN users s ON s.id = b.user_id \
     LEFT JOIN class_participants cp ON cp.user_id = s.id AND cp.is_primary \
     LEFT JOIN classes cl ON cl.id = cp.class_id \
     LEFT JOIN users v ON v.id = b.verified_by";

fn row_to_bill(r: &Row) -> BillItem {
    let paid_at: Option<chrono::DateTime<Utc>> = r.get(10);
    BillItem {
        id: r.get(0),
        user_id: r.get(1),
        student_name: r.get(2),
        nis: r.get(3),
        class_name: r.get(4),
        title: r.get(5),
        price: r.get(6),
        started_date: r.get::<_, NaiveDate>(7).to_string(),
        expired_date: r.get::<_, NaiveDate>(8).to_string(),
        status: r.get(9),
        paid_at: paid_at
            .map(|t| {
                t.with_timezone(&chrono::FixedOffset::east_opt(7 * 3600).unwrap())
                    .format("%d %b %Y %H:%M")
                    .to_string()
            })
            .unwrap_or_default(),
        paid_amount: r.get(11),
        method: r.get(12),
        proof_url: r.get(13),
        verified_by_name: r.get(14),
        note: r.get(15),
        overdue: r.get(16),
    }
}

/// Semua tagihan BELUM lunas (untuk finance: admin/ketua/santri_finance).
pub async fn list_unpaid(pool: &Pool, limit: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    let sql = format!("{BILL_SELECT} WHERE b.status = 'belum' ORDER BY b.expired_date, s.full_name LIMIT $1");
    let rows = c.query(&sql, &[&limit]).await.context("list_unpaid")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

/// Tagihan milik satu santri (dashboard santri).
pub async fn list_for_user(pool: &Pool, user_id: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    let sql = format!("{BILL_SELECT} WHERE b.user_id = $1 ORDER BY b.expired_date DESC");
    let rows = c.query(&sql, &[&user_id]).await.context("list_for_user")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

/// Buat tagihan. Tanggal "YYYY-MM-DD".
pub async fn create_bill(
    pool: &Pool,
    user_id: i64,
    title: &str,
    price: i64,
    started: NaiveDate,
    expired: NaiveDate,
    note: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO bills (user_id, title, price, started_date, expired_date, note) \
             VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
            &[&user_id, &title, &price, &started, &expired, &note],
        )
        .await
        .context("create_bill")?;
    Ok(row.get(0))
}

/// Tandai LUNAS + verifikasi (finance). paid_amount default = price bila None.
pub async fn mark_paid(
    pool: &Pool,
    bill_id: i64,
    paid_amount: Option<i64>,
    method: &str,
    verified_by: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE bills SET status='lunas', paid_at=now(), \
                    paid_amount = COALESCE($2, price), method=$3, verified_by=$4 \
             WHERE id=$1",
            &[&bill_id, &paid_amount, &method, &verified_by],
        )
        .await
        .context("mark_paid")?;
    Ok(n > 0)
}

/// Santri unggah bukti bayar (guard: hanya tagihannya sendiri).
pub async fn set_proof(pool: &Pool, bill_id: i64, user_id: i64, proof_url: &str) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE bills SET proof_url=$3 WHERE id=$1 AND user_id=$2",
            &[&bill_id, &user_id, &proof_url],
        )
        .await
        .context("set_proof")?;
    Ok(n > 0)
}

/// Hapus tagihan (admin/ketua).
pub async fn delete_bill(pool: &Pool, bill_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM bills WHERE id=$1", &[&bill_id])
        .await
        .context("delete_bill")?;
    Ok(n > 0)
}
