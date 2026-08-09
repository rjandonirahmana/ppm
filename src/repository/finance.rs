//! repository/finance.rs — Tagihan santri (migrasi 37).

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::models::BillItem;

/// Kolom + join standar → BillItem. `s` = santri, `v` = verifikator,
/// `cl` = kelas AKADEMIK santri (lihat `kelas_utama_lateral`).
///
/// Fungsi, bukan const: join kelasnya kini LATERAL yang disusun saat runtime.
fn bill_select() -> String {
    let kelas = super::kelas_utama_lateral("s.id");
    format!(
        "SELECT b.id, b.user_id, s.full_name, COALESCE(s.nis,'-'), \
        COALESCE(cl.name,'-'), b.title, b.price, b.started_date, b.expired_date, \
        b.status, b.paid_at, b.paid_amount, COALESCE(b.method,''), COALESCE(b.proof_url,''), \
        COALESCE(v.full_name,''), b.note, \
        (b.status = 'belum' AND b.expired_date IS NOT NULL \
            AND b.expired_date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date) AS overdue, \
        COALESCE(b.reject_reason,''), b.submitted_at, COALESCE(p.full_name,'') \
     FROM bills b \
     JOIN users s ON s.id = b.user_id \
     {kelas} \
     LEFT JOIN users v ON v.id = b.verified_by \
     LEFT JOIN users p ON p.id = b.submitted_by"
    )
}

/// Tanggal yang BOLEH kosong (migrasi 75: periode diisi saat verifikasi) →
/// string ISO atau "". Layar membedakan keduanya sendiri; yang penting di sini
/// adalah `r.get::<_, NaiveDate>` TIDAK dipakai lagi — pada baris pengajuan
/// yang belum diverifikasi kolomnya NULL dan pembacaan itu akan panik.
fn tanggal_opsional(r: &Row, idx: usize) -> String {
    r.get::<_, Option<NaiveDate>>(idx).map(|d| d.to_string()).unwrap_or_default()
}

fn row_to_bill(r: &Row) -> BillItem {
    let paid_at: Option<chrono::DateTime<Utc>> = r.get(10);
    let submitted_at: Option<chrono::DateTime<Utc>> = r.get(18);
    BillItem {
        id: r.get(0),
        user_id: r.get(1),
        student_name: r.get(2),
        nis: r.get(3),
        class_name: r.get(4),
        title: r.get(5),
        price: r.get(6),
        started_date: tanggal_opsional(r, 7),
        expired_date: tanggal_opsional(r, 8),
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
        reject_reason: r.get(17),
        submitted_at: submitted_at
            .map(|t| {
                t.with_timezone(&chrono::FixedOffset::east_opt(7 * 3600).unwrap())
                    .format("%d %b %Y %H:%M")
                    .to_string()
            })
            .unwrap_or_default(),
        submitted_by_name: r.get(19),
    }
}

/// Semua tagihan BELUM lunas (untuk finance: admin/ketua/santri_finance).
pub async fn list_unpaid(pool: &Pool, limit: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    let sql = format!("{} WHERE b.status = 'belum' ORDER BY b.expired_date, s.full_name LIMIT $1", bill_select());
    let rows = c.query(&sql, &[&limit]).await.context("list_unpaid")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

/// Riwayat pembayaran: tagihan LUNAS semua santri, terbaru dibayar dulu
/// (untuk finance). Menyertakan periode, nominal, metode, bukti TF, verifikator.
pub async fn list_paid(pool: &Pool, limit: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    let sql = format!(
        "{} WHERE b.status = 'lunas' \
         ORDER BY b.paid_at DESC NULLS LAST, s.full_name LIMIT $1",
        bill_select()
    );
    let rows = c.query(&sql, &[&limit]).await.context("list_paid")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

/// Tagihan milik satu santri (dashboard santri).
pub async fn list_for_user(pool: &Pool, user_id: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    let sql = format!("{} WHERE b.user_id = $1 ORDER BY b.expired_date DESC", bill_select());
    let rows = c.query(&sql, &[&user_id]).await.context("list_for_user")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

/// Pembayaran yang SUDAH terjadi, dicatat bersamaan dengan periodenya.
///
/// Pengurus sering memasukkan setoran yang sudah lama diterima (uang tunai di
/// meja, transfer bulan lalu). Sebelumnya jalannya dua langkah — buat periode
/// "belum", lalu tandai lunas — dan di antara keduanya catatan itu muncul di
/// daftar periode berjalan seolah santri menunggak. Menyatukannya juga membuat
/// pencatatan atomik: tak ada baris setengah jadi bila langkah kedua gagal.
#[derive(Clone, Copy, Debug)]
pub struct PembayaranTercatat {
    /// "transfer" | "tunai".
    pub method: &'static str,
    /// Tanggal uang diterima (bukan tanggal input). Jam disetel tengah hari
    /// WIB: tanggalnya yang bermakna, dan tengah hari aman dari pergeseran
    /// zona waktu ke tanggal sebelah saat ditampilkan.
    pub paid_date: NaiveDate,
    /// Pengurus yang mencatat.
    pub verified_by: i64,
}

/// Buat catatan pembayaran (periode + nominal). `paid` terisi = langsung
/// tersimpan LUNAS, jadi ia masuk Riwayat Pembayaran, bukan periode berjalan.
pub async fn create_bill(
    pool: &Pool,
    user_id: i64,
    title: &str,
    price: i64,
    started: NaiveDate,
    expired: NaiveDate,
    note: &str,
    paid: Option<PembayaranTercatat>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = match paid {
        None => {
            c.query_one(
                "INSERT INTO bills (user_id, title, price, started_date, expired_date, note) \
                 VALUES ($1,$2,$3,$4,$5,$6) RETURNING id",
                &[&user_id, &title, &price, &started, &expired, &note],
            )
            .await
        }
        Some(p) => {
            // Tengah hari WIB = 05:00 UTC — lihat alasan di `PembayaranTercatat`.
            let paid_at = p
                .paid_date
                .and_hms_opt(5, 0, 0)
                .map(|t| chrono::DateTime::<Utc>::from_naive_utc_and_offset(t, Utc))
                .unwrap_or_else(Utc::now);
            c.query_one(
                "INSERT INTO bills (user_id, title, price, started_date, expired_date, note, \
                                    status, paid_at, paid_amount, method, verified_by) \
                 VALUES ($1,$2,$3,$4,$5,$6,'lunas',$7,$3,$8,$9) RETURNING id",
                &[
                    &user_id,
                    &title,
                    &price,
                    &started,
                    &expired,
                    &note,
                    &paid_at,
                    &p.method,
                    &p.verified_by,
                ],
            )
            .await
        }
    }
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

// ── Pengajuan pembayaran oleh santri / orang tua (migrasi 75) ────────────────

/// Catat pengajuan: keluarga menyetor sejumlah uang + bukti transfer, periode
/// menyusul saat diverifikasi.
///
/// `price` diisi nominal yang DIAKUI penyetor; `paid_amount` sengaja dibiarkan
/// NULL sampai verifikator mencocokkannya dengan mutasi rekening. Selisih di
/// antara keduanya itulah yang perlu terlihat.
pub async fn ajukan_pembayaran(
    pool: &Pool,
    student_id: i64,
    submitted_by: i64,
    amount: i64,
    proof_url: &str,
    catatan: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO bills (user_id, title, price, status, proof_url, note, \
                                submitted_by, submitted_at) \
             VALUES ($1, 'Pengajuan pembayaran', $2, 'menunggu', $3, $4, $5, NOW()) \
             RETURNING id",
            &[&student_id, &amount, &proof_url, &catatan, &submitted_by],
        )
        .await
        .context("ajukan_pembayaran")?;
    Ok(row.get(0))
}

/// Antrean verifikasi — terlama dulu, supaya tak ada yang menunggu selamanya
/// hanya karena pengajuan baru terus menyalip di atasnya.
pub async fn list_menunggu(pool: &Pool, limit: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    let sql = format!(
        "{} WHERE b.status = 'menunggu' ORDER BY b.submitted_at NULLS LAST LIMIT $1",
        bill_select()
    );
    let rows = c.query(&sql, &[&limit]).await.context("list_menunggu")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

/// Setujui pengajuan: tetapkan periode berlakunya + nominal yang benar-benar
/// masuk. Hanya baris berstatus `menunggu` yang tersentuh — guard itu yang
/// membuat klik ganda (atau dua petugas bersamaan) tak menimpa jejak
/// verifikator pertama.
#[allow(clippy::too_many_arguments)]
pub async fn setujui_pengajuan(
    pool: &Pool,
    bill_id: i64,
    judul: &str,
    started: NaiveDate,
    expired: NaiveDate,
    paid_amount: i64,
    method: &str,
    verified_by: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE bills SET status='lunas', title=$2, started_date=$3, expired_date=$4, \
                    paid_amount=$5, method=$6, verified_by=$7, paid_at=NOW(), \
                    reject_reason=NULL \
              WHERE id=$1 AND status='menunggu'",
            &[&bill_id, &judul, &started, &expired, &paid_amount, &method, &verified_by],
        )
        .await
        .context("setujui_pengajuan")?;
    Ok(n > 0)
}

/// Tolak pengajuan (mis. tak ada mutasi masuk yang cocok). Barisnya DISIMPAN,
/// tidak dihapus: keluarga perlu melihat bahwa kirimannya sudah diperiksa dan
/// kenapa ditolak — pengajuan yang lenyap tanpa jejak terbaca sebagai aplikasi
/// yang menelan bukti transfer.
pub async fn tolak_pengajuan(
    pool: &Pool,
    bill_id: i64,
    alasan: &str,
    verified_by: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE bills SET status='ditolak', reject_reason=$2, verified_by=$3 \
              WHERE id=$1 AND status='menunggu'",
            &[&bill_id, &alasan, &verified_by],
        )
        .await
        .context("tolak_pengajuan")?;
    Ok(n > 0)
}

/// Pengajuan milik satu santri — dipakai layar santri & orang tua untuk
/// menampilkan status kirimannya (menunggu / lunas / ditolak beserta alasannya).
pub async fn list_for_student(pool: &Pool, student_id: i64, limit: i64) -> Result<Vec<BillItem>> {
    let c = pool.get().await?;
    // Yang paling perlu dilihat keluarga adalah kiriman terakhirnya, jadi
    // urutannya "kapan barisnya muncul" — pengajuan pakai submitted_at, catatan
    // langsung dari pengurus pakai created_at.
    let sql = format!(
        "{} WHERE b.user_id = $1 \
         ORDER BY COALESCE(b.submitted_at, b.created_at) DESC LIMIT $2",
        bill_select()
    );
    let rows = c.query(&sql, &[&student_id, &limit]).await.context("list_for_student")?;
    Ok(rows.iter().map(row_to_bill).collect())
}

// ── Periode terlewat + pengingat WhatsApp (migrasi 75) ───────────────────────

pub struct TunggakanRow {
    pub user_id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub habis: Option<NaiveDate>,
    pub punya_hp: bool,
    pub jumlah_ortu: i64,
    pub diingatkan: Option<chrono::DateTime<Utc>>,
}

/// Santri aktif yang periode bayarnya SUDAH HABIS, atau belum pernah tercatat.
///
/// Satu query untuk kedua kelompok — pemisahannya dilakukan di service dari
/// `habis IS NULL`. Dua query terpisah akan membaca tabel `users` dua kali
/// untuk perbedaan satu kolom.
///
/// LATERAL, bukan `GROUP BY`: yang dicari cuma SATU baris per santri (periode
/// lunas dengan `expired_date` terbesar), dan `idx_bills_lunas_per_santri`
/// melayaninya langsung tanpa mengagregasi seluruh riwayat pembayaran pondok.
pub async fn periode_terlewat(pool: &Pool) -> Result<Vec<TunggakanRow>> {
    let c = pool.get().await?;
    let kelas = super::kelas_utama_lateral("u.id");
    let sql = format!(
        "SELECT u.id, u.full_name, u.nis, cl.name, akhir.expired_date, \
                COALESCE(u.phone_number, '') <> '' AS punya_hp, \
                (SELECT COUNT(*) FROM parent_connections pc \
                   JOIN users o ON o.id = pc.parent_id \
                  WHERE pc.student_id = u.id AND pc.status = 'connected' \
                    AND COALESCE(o.phone_number,'') <> '')::bigint AS jumlah_ortu, \
                u.bill_reminded_at \
           FROM users u \
           {kelas} \
           LEFT JOIN LATERAL ( \
                SELECT b.expired_date FROM bills b \
                 WHERE b.user_id = u.id AND b.status = 'lunas' \
                   AND b.expired_date IS NOT NULL \
                 ORDER BY b.expired_date DESC LIMIT 1 \
           ) akhir ON TRUE \
          WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
            AND (akhir.expired_date IS NULL \
                 OR akhir.expired_date < (NOW() AT TIME ZONE 'Asia/Jakarta')::date) \
          ORDER BY akhir.expired_date NULLS LAST, u.full_name"
    );
    let rows = c.query(&sql, &[]).await.context("periode_terlewat")?;
    Ok(rows
        .into_iter()
        .map(|r| TunggakanRow {
            user_id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            habis: r.get(4),
            punya_hp: r.get(5),
            jumlah_ortu: r.get(6),
            diingatkan: r.get(7),
        })
        .collect())
}

/// Nomor tujuan pengingat untuk sekumpulan santri: nomor santri sendiri +
/// SEMUA orang tua yang terhubung.
pub struct TujuanWa {
    pub user_id: i64,
    pub student_name: String,
    /// Nomor mentah (belum dinormalisasi ke 62xxx).
    pub nomor: Vec<String>,
}

pub async fn tujuan_pengingat(pool: &Pool, ids: &[i64]) -> Result<Vec<TujuanWa>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, \
                    ARRAY_REMOVE(ARRAY_APPEND( \
                        ARRAY( SELECT o.phone_number FROM parent_connections pc \
                                 JOIN users o ON o.id = pc.parent_id \
                                WHERE pc.student_id = u.id AND pc.status = 'connected' \
                                  AND COALESCE(o.phone_number,'') <> '' ), \
                        NULLIF(u.phone_number, '')), NULL) AS nomor \
               FROM users u \
              WHERE u.id = ANY($1) AND u.is_active = TRUE",
            &[&ids],
        )
        .await
        .context("tujuan_pengingat")?;
    Ok(rows
        .into_iter()
        .map(|r| TujuanWa { user_id: r.get(0), student_name: r.get(1), nomor: r.get(2) })
        .collect())
}

/// Tandai kapan pengingat terakhir dikirim — supaya keluarga yang sama tak
/// ditagih berkali-kali oleh pengurus yang berbeda.
pub async fn tandai_diingatkan(pool: &Pool, ids: &[i64]) -> Result<u64> {
    let c = pool.get().await?;
    c.execute("UPDATE users SET bill_reminded_at = NOW() WHERE id = ANY($1)", &[&ids])
        .await
        .context("tandai_diingatkan")
}

/// Pemilik satu tagihan — dipakai menolak verifikasi-diri-sendiri oleh
/// santri_finance (lihat web/api.rs::mark_bill_paid_action).
pub async fn bill_owner(pool: &Pool, bill_id: i64) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT user_id FROM bills WHERE id = $1", &[&bill_id])
        .await
        .context("bill_owner")?;
    Ok(row.map(|r| r.get(0)))
}
