//! repository/notifications.rs — Query notifikasi dalam aplikasi.
//!
//! Skema & alasan bentuknya ada di `migration/92_notifikasi.sql`.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

/// Satu notifikasi yang akan ditulis.
pub struct NotifBaru {
    pub user_id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
}

pub struct NotifRow {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub link: Option<String>,
    pub read_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Tulis banyak notifikasi dalam SATU perjalanan ke database.
///
/// ── KENAPA UNNEST, BUKAN LOOP INSERT ───────────────────────────────────────
/// Satu pengajuan izin memberi tahu wali kelas DAN semua admin sekaligus. Loop
/// `INSERT` berarti satu round-trip per penerima, dan round-trip itulah
/// biayanya — bukan penulisan barisnya. Dengan lima admin, itu enam perjalanan
/// bolak-balik yang menahan koneksi pool selama pengajuan berlangsung.
///
/// `UNNEST` mengirim seluruh larik sekaligus: satu perjalanan, satu rencana
/// query, berapa pun penerimanya. Ia juga membuat penulisannya atomik dengan
/// sendirinya — tak ada keadaan setengah jadi di mana wali kelas dapat
/// notifikasi tapi admin tidak.
///
/// Best-effort di sisi pemanggil: notifikasi yang gagal ditulis TIDAK boleh
/// menggagalkan pengajuan izinnya (lihat `service::notifications`).
pub async fn notif_insert_many(pool: &Pool, items: &[NotifBaru]) -> Result<u64> {
    if items.is_empty() {
        return Ok(0);
    }
    let c = pool.get().await?;

    let user_ids: Vec<i64> = items.iter().map(|n| n.user_id).collect();
    let kinds: Vec<&str> = items.iter().map(|n| n.kind.as_str()).collect();
    let titles: Vec<&str> = items.iter().map(|n| n.title.as_str()).collect();
    let bodies: Vec<&str> = items.iter().map(|n| n.body.as_str()).collect();
    // `Option<&str>` supaya NULL tetap NULL, bukan string kosong: pembacanya
    // membedakan "tak ke mana-mana" dari "tujuan kosong".
    let links: Vec<Option<&str>> = items.iter().map(|n| n.link.as_deref()).collect();

    let n = c
        .execute(
            "INSERT INTO notifications (user_id, kind, title, body, link) \
             SELECT * FROM UNNEST($1::bigint[], $2::text[], $3::text[], $4::text[], $5::text[])",
            &[&user_ids, &kinds, &titles, &bodies, &links],
        )
        .await
        .context("notifications insert_many")?;
    Ok(n)
}

/// Feed lonceng: `limit` notifikasi terbaru milik satu orang.
///
/// Index `idx_notifications_user_baru` sudah berurut `(user_id, created_at
/// DESC)`, jadi ini index scan yang berhenti setelah `limit` baris — biayanya
/// tak bertambah seiring tabelnya tumbuh.
pub async fn notif_list_for_user(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<NotifRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, kind, title, body, link, read_at, created_at \
               FROM notifications \
              WHERE user_id = $1 \
              ORDER BY created_at DESC \
              LIMIT $2",
            &[&user_id, &limit],
        )
        .await
        .context("notifications list_for_user")?;
    Ok(rows
        .into_iter()
        .map(|r| NotifRow {
            id: r.get(0),
            kind: r.get(1),
            title: r.get(2),
            body: r.get(3),
            link: r.get(4),
            read_at: r.get(5),
            created_at: r.get(6),
        })
        .collect())
}

/// Jumlah yang belum dibaca.
///
/// Dijawab index parsial `idx_notifications_belum_dibaca`, yang hanya memuat
/// baris belum-dibaca — jadi hitungannya tak pernah menyentuh riwayat lama.
pub async fn notif_unread_count(pool: &Pool, user_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
            &[&user_id],
        )
        .await
        .context("notifications unread_count")?;
    Ok(row.get(0))
}

/// Tandai satu notifikasi terbaca.
///
/// `user_id` ada di WHERE, bukan cuma `id`: tanpa itu siapa pun yang menebak
/// nomor bisa menandai notifikasi orang lain. Kepemilikan ditegakkan di query,
/// bukan dipercayakan ke pemanggil.
///
/// `AND read_at IS NULL` membuatnya idempoten — mengetuk dua kali tidak
/// menggeser waktu bacanya.
pub async fn notif_mark_read(pool: &Pool, user_id: i64, id: i64) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE notifications SET read_at = NOW() \
          WHERE id = $1 AND user_id = $2 AND read_at IS NULL",
        &[&id, &user_id],
    )
    .await
    .context("notifications mark_read")?;
    Ok(())
}

/// Tandai semua milik satu orang terbaca.
pub async fn notif_mark_all_read(pool: &Pool, user_id: i64) -> Result<u64> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE notifications SET read_at = NOW() \
              WHERE user_id = $1 AND read_at IS NULL",
            &[&user_id],
        )
        .await
        .context("notifications mark_all_read")?;
    Ok(n)
}

/// Id semua admin — penerima tetap tiap pengajuan izin.
///
/// Query terpisah dan sengaja sempit: hanya kolom `id`, karena yang dibutuhkan
/// hanya itu. Jumlah admin di pesantren ini hitungan jari, jadi tak ada
/// paginasi — tapi `LIMIT` tetap dipasang sebagai pagar, supaya salah data
/// (misalnya seluruh akun ter-set 'admin') tak berubah menjadi puluhan ribu
/// baris notifikasi dari satu pengajuan.
pub async fn notif_admin_ids(pool: &Pool) -> Result<Vec<i64>> {
    let c = pool.get().await?;
    let rows = c
        .query("SELECT id FROM users WHERE role = 'admin' LIMIT 50", &[])
        .await
        .context("notifications admin_ids")?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}
