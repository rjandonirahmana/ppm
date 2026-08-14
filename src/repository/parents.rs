//! repository/parents.rs — Query koneksi orang tua ↔ santri (parent_connections).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use deadpool_postgres::Pool;

/// Cari santri berdasar nama (ILIKE) atau NIS persis. Untuk form koneksi ortu.
pub struct StudentRow {
    pub id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
}

pub async fn search_students(pool: &Pool, q: &str, limit: i64) -> Result<Vec<StudentRow>> {
    let c = pool.get().await?;
    let pattern = format!("%{}%", q.trim());
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, c.name \
             FROM users u \
             LEFT JOIN classes c ON c.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
               AND (u.full_name ILIKE $1 OR u.nis = $2) \
             ORDER BY u.full_name LIMIT $3",
            &[&pattern, &q.trim(), &limit],
        )
        .await
        .context("search_students")?;
    Ok(rows
        .into_iter()
        .map(|r| StudentRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
        })
        .collect())
}

pub struct ConnRow {
    pub id: i64,
    pub student_id: i64,
    pub student_name: String,
    pub status: String,
    pub requested_at: DateTime<Utc>,
}

/// Semua koneksi milik satu orang tua (connected + pending).
pub async fn connections_of_parent(pool: &Pool, parent_id: i64) -> Result<Vec<ConnRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT pc.id, pc.student_id, u.full_name, pc.status, pc.requested_at \
             FROM parent_connections pc \
             JOIN users u ON u.id = pc.student_id \
             WHERE pc.parent_id = $1 AND pc.status IN ('pending','connected') \
             ORDER BY pc.requested_at ASC",
            &[&parent_id],
        )
        .await
        .context("connections_of_parent")?;
    Ok(rows
        .into_iter()
        .map(|r| ConnRow {
            id: r.get(0),
            student_id: r.get(1),
            student_name: r.get(2),
            status: r.get(3),
            requested_at: r.get(4),
        })
        .collect())
}

/// Kirim permintaan koneksi. Return false bila sudah ada (pending/connected).
pub async fn insert_connection(pool: &Pool, parent_id: i64, student_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO parent_connections (parent_id, student_id) VALUES ($1, $2) \
             ON CONFLICT (parent_id, student_id) DO NOTHING",
            &[&parent_id, &student_id],
        )
        .await
        .context("insert_connection")?;
    Ok(n > 0)
}

/// Apakah ortu terhubung (connected) ke santri ini? Guard akses data anak.
pub async fn is_connected(pool: &Pool, parent_id: i64, student_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT 1 FROM parent_connections \
             WHERE parent_id = $1 AND student_id = $2 AND status = 'connected'",
            &[&parent_id, &student_id],
        )
        .await?;
    Ok(row.is_some())
}

pub struct IncomingReq {
    pub id: i64,
    pub parent_name: String,
    pub requested_at: DateTime<Utc>,
}

/// Permintaan koneksi MASUK untuk seorang santri (menunggu persetujuannya).
pub async fn pending_for_student(pool: &Pool, student_id: i64) -> Result<Vec<IncomingReq>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT pc.id, u.full_name, pc.requested_at \
             FROM parent_connections pc \
             JOIN users u ON u.id = pc.parent_id \
             WHERE pc.student_id = $1 AND pc.status = 'pending' \
             ORDER BY pc.requested_at ASC",
            &[&student_id],
        )
        .await
        .context("pending_for_student")?;
    Ok(rows
        .into_iter()
        .map(|r| IncomingReq {
            id: r.get(0),
            parent_name: r.get(1),
            requested_at: r.get(2),
        })
        .collect())
}

/// Santri menyetujui/menolak permintaan. Return true bila ada yang ter-update.
pub async fn respond_connection(
    pool: &Pool,
    conn_id: i64,
    student_id: i64,
    approve: bool,
) -> Result<bool> {
    let c = pool.get().await?;
    let status = if approve { "connected" } else { "rejected" };
    let n = c
        .execute(
            "UPDATE parent_connections SET status = $3, responded_at = NOW() \
             WHERE id = $1 AND student_id = $2 AND status = 'pending'",
            &[&conn_id, &student_id, &status],
        )
        .await
        .context("respond_connection")?;
    Ok(n > 0)
}

// ── Penyambungan oleh PENGELOLA (admin/ketua, halaman manajemen user) ────────
//
// Alur biasa menuntut persetujuan santri: ortu mencari anaknya, santri menekan
// "Setujui". Itu benar sebagai bawaan — koneksi memberi akses ke data pribadi
// seseorang, dan yang berhak mengizinkannya adalah orangnya sendiri.
//
// Tapi alur itu buntu untuk sebagian keluarga: santri yang belum pernah membuka
// aplikasi tak bisa menyetujui apa pun, sementara wali sudah menunggu. Selama
// ini satu-satunya jalan keluarnya adalah UPDATE langsung ke produksi. Karena
// pengelola sudah dipercaya mengubah peran dan menonaktifkan akun, ia juga
// boleh menetapkan hubungan ini. Jalur santri TIDAK dihapus — ini pintu kedua,
// bukan penggantinya.

/// Satu anak yang terhubung (atau menunggu) ke seorang ortu, dengan identitas
/// pembeda: pada daftar induk ada puluhan nama kembar, dan pengelola harus bisa
/// memastikan ia melepas/menyambung anak yang benar tanpa membuka halaman lain.
pub struct AnakRow {
    pub student_id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    /// "pending" | "connected".
    pub status: String,
}

/// Anak-anak seorang ortu untuk layar pengelola (pending + connected).
pub async fn children_of_parent(pool: &Pool, parent_id: i64) -> Result<Vec<AnakRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT pc.student_id, u.full_name, u.nis, cl.name, pc.status \
             FROM parent_connections pc \
             JOIN users u ON u.id = pc.student_id \
             LEFT JOIN classes cl ON cl.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE pc.parent_id = $1 AND pc.status IN ('pending','connected') \
             ORDER BY (pc.status = 'connected') DESC, u.full_name",
            &[&parent_id],
        )
        .await
        .context("children_of_parent")?;
    Ok(rows
        .into_iter()
        .map(|r| AnakRow {
            student_id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            status: r.get(4),
        })
        .collect())
}

/// Satu ORANG TUA dari sudut pandang santri — cerminan [`AnakRow`].
pub struct OrtuRow {
    pub parent_id: i64,
    pub full_name: String,
    pub phone_number: Option<String>,
    /// "pending" | "connected".
    pub status: String,
}

/// Orang tua seorang santri untuk layar pengelola (pending + connected).
///
/// Relasinya MEMANG banyak-ke-banyak: satu santri bisa punya ayah dan ibu (atau
/// wali) dengan akun masing-masing, dan satu akun ortu bisa punya beberapa anak
/// di pondok yang sama. `parent_connections` sejak awal berbentuk junction
/// dengan UNIQUE(parent_id, student_id) — jadi tak ada satu pun batas yang perlu
/// dilonggarkan di sini; yang selama ini kurang hanyalah layar dari SISI SANTRI.
pub async fn parents_of_student(pool: &Pool, student_id: i64) -> Result<Vec<OrtuRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT pc.parent_id, u.full_name, u.phone_number, pc.status \
             FROM parent_connections pc \
             JOIN users u ON u.id = pc.parent_id \
             WHERE pc.student_id = $1 AND pc.status IN ('pending','connected') \
             ORDER BY (pc.status = 'connected') DESC, u.full_name",
            &[&student_id],
        )
        .await
        .context("parents_of_student")?;
    Ok(rows
        .into_iter()
        .map(|r| OrtuRow {
            parent_id: r.get(0),
            full_name: r.get(1),
            phone_number: r.get(2),
            status: r.get(3),
        })
        .collect())
}

/// Cari akun ORANG TUA berdasar nama (ILIKE) atau nomor HP.
///
/// Nomor dicocokkan dengan `LIKE '%…'` atas digitnya saja: satu nomor yang sama
/// tersimpan bisa berbentuk `08…`, `62…`, atau `+62…` tergantung siapa yang
/// mengetiknya, dan pengelola yang menyalin nomor dari WhatsApp tak boleh gagal
/// menemukan orangnya hanya karena awalannya berbeda.
pub async fn search_parents(pool: &Pool, q: &str, limit: i64) -> Result<Vec<OrtuRow>> {
    let c = pool.get().await?;
    let q = q.trim();
    let nama = format!("%{q}%");
    let digit: String = q.chars().filter(|c| c.is_ascii_digit()).collect();
    // 4 digit terakhir sudah cukup menyaring, dan lebih pendek dari itu akan
    // mengembalikan hampir semua orang.
    let ekor = if digit.len() >= 4 {
        Some(format!("%{}", &digit[digit.len().saturating_sub(8)..]))
    } else {
        None
    };
    let rows = c
        .query(
            // Status 'belum' — BUKAN 'connected'. Hasil pencarian adalah calon
            // yang belum tertaut apa pun; memberinya label "Terhubung" (yang
            // dilakukan versi pertama) berarti layar berbohong tentang keadaan
            // yang justru sedang hendak diubah pengelola.
            "SELECT u.id, u.full_name, u.phone_number, 'belum' \
             FROM users u \
             WHERE u.role = 'parent' AND u.is_active = TRUE \
               AND (u.full_name ILIKE $1 \
                    OR ($2::text IS NOT NULL \
                        AND regexp_replace(COALESCE(u.phone_number,''), '\\D', '', 'g') LIKE $2)) \
             ORDER BY u.full_name LIMIT $3",
            &[&nama, &ekor, &limit],
        )
        .await
        .context("search_parents")?;
    Ok(rows
        .into_iter()
        .map(|r| OrtuRow {
            parent_id: r.get(0),
            full_name: r.get(1),
            phone_number: r.get(2),
            status: r.get(3),
        })
        .collect())
}

/// Sambungkan ortu ↔ santri LANGSUNG (status `connected`) atas nama pengelola.
///
/// Idempotent: permintaan yang sudah ada — termasuk yang pernah `rejected` —
/// diangkat menjadi `connected`, bukan gagal karena UNIQUE. Pengelola yang
/// menyambungkan ulang sesudah salah tolak tak perlu tahu ada baris lama.
pub async fn admin_link_child(pool: &Pool, parent_id: i64, student_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO parent_connections (parent_id, student_id, status, responded_at) \
             VALUES ($1, $2, 'connected', NOW()) \
             ON CONFLICT (parent_id, student_id) DO UPDATE \
                SET status = 'connected', responded_at = NOW() \
              WHERE parent_connections.status <> 'connected'",
            &[&parent_id, &student_id],
        )
        .await
        .context("admin_link_child")?;
    Ok(n > 0)
}

/// Putuskan hubungan ortu ↔ santri (dihapus, bukan ditandai ditolak).
///
/// Dihapus supaya pengelola yang keliru bisa menyambungkan ulang lewat jalur
/// mana pun — termasuk permintaan baru dari ortu — tanpa menabrak baris lama
/// berstatus `rejected` yang tak terlihat di layar mana pun.
pub async fn admin_unlink_child(pool: &Pool, parent_id: i64, student_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "DELETE FROM parent_connections WHERE parent_id = $1 AND student_id = $2",
            &[&parent_id, &student_id],
        )
        .await
        .context("admin_unlink_child")?;
    Ok(n > 0)
}

/// Info dasar anak (nama, nis, kelas utama).
pub async fn child_info(pool: &Pool, student_id: i64) -> Result<Option<StudentRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT u.id, u.full_name, u.nis, c.name \
             FROM users u \
             LEFT JOIN classes c ON c.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE u.id = $1 AND u.role IN ('santri', 'santri_finance')",
            &[&student_id],
        )
        .await?;
    Ok(row.map(|r| StudentRow {
        id: r.get(0),
        full_name: r.get(1),
        nis: r.get(2),
        class_name: r.get(3),
    }))
}

pub struct ParentPermitRow {
    pub id: i64,
    /// Diajukan ORANG TUA atas nama anaknya (bukan santri sendiri).
    pub oleh_ortu: bool,
    pub requester_name: String,
    pub child_name: String,
    pub kind: String,
    pub mulai: chrono::NaiveDateTime,
    pub selesai: chrono::NaiveDateTime,
    pub reason: String,
    pub guru_status: String,
    pub created_at: DateTime<Utc>,
}

/// Semua izin milik anak-anak yang terhubung ke ortu ini (terbaru dulu).
pub async fn permits_of_children(pool: &Pool, parent_id: i64, limit: i64) -> Result<Vec<ParentPermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            &format!(
                "SELECT u.full_name, p.type, p.start_time, p.end_time, p.reason, \
                        p.guru_status, p.created_at, \
                        p.id, (p.requested_by <> p.user_id AND rb.role = 'parent'), \
                        COALESCE(rb.full_name, '') \
                 FROM permit_requests p \
                 JOIN parent_connections pc ON pc.student_id = p.user_id \
                      AND pc.parent_id = $1 AND pc.status = 'connected' \
                 JOIN users u ON u.id = p.user_id \
                 LEFT JOIN users rb ON rb.id = p.requested_by \
                 LEFT JOIN classes tc ON tc.id = p.class_id \
                 {kelas} \
                 ORDER BY p.created_at DESC LIMIT $2",
                kelas = super::kelas_utama_lateral("p.user_id"),
            ),
            &[&parent_id, &limit],
        )
        .await
        .context("permits_of_children")?;
    Ok(rows
        .into_iter()
        .map(|r| ParentPermitRow {
            child_name: r.get(0),
            kind: r.get(1),
            mulai: r.get(2),
            selesai: r.get(3),
            reason: r.get(4),
            guru_status: r.get(5),
            created_at: r.get(6),
            id: r.get(7),
            oleh_ortu: r.get(8),
            requester_name: r.get(9),
        })
        .collect())
}
