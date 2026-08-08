//! repository/users.rs — Query tabel users.

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

pub struct LoginRow {
    pub id: i64,
    pub full_name: String,
    pub role: String,
    pub password_hash: String,
    pub phone_number: Option<String>,
}

/// Cari user untuk login — UTAMANYA nomor HP (login = phone), tetap dukung
/// username/email/NIS sbg fallback (mis. admin seed). `login_phone` = HP hasil
/// normalisasi 08→62. Pencocokan HP toleran terhadap format tersimpan yang
/// kotor ('+62 858-…', '0858…'): digit-nya dibandingkan dengan bentuk 62.. dan
/// 0.. — data lama/seed tidak ternormalisasi konsisten. ORDER BY + LIMIT
/// supaya deterministik bila identitas kebetulan cocok di >1 baris (mis. NIS
/// satu santri sama dengan username user lain).
pub async fn find_user_for_login(
    pool: &Pool,
    login: &str,
    login_phone: &str,
) -> Result<Option<LoginRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, full_name, role, password_hash, phone_number FROM users \
             WHERE (username = $1 OR email = $1 OR nis = $1 \
                    OR phone_number = $1 OR phone_number = $2 \
                    OR ($2 <> '' AND regexp_replace(coalesce(phone_number, ''), '\\D', '', 'g') \
                        IN ($2, '0' || substring($2 from 3)))) \
               AND is_active = TRUE \
             ORDER BY id LIMIT 1",
            &[&login, &login_phone],
        )
        .await
        .context("find_user_for_login")?;
    Ok(row.map(|r| LoginRow {
        id: r.get(0),
        full_name: r.get(1),
        role: r.get(2),
        password_hash: r.get(3),
        phone_number: r.get(4),
    }))
}

pub struct UserHome {
    pub full_name: String,
    pub points: i32,
}

pub async fn user_home(pool: &Pool, user_id: i64) -> Result<Option<UserHome>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT full_name, points FROM users WHERE id = $1", &[&user_id])
        .await?;
    Ok(row.map(|r| UserHome {
        full_name: r.get(0),
        points: r.get(1),
    }))
}

/// Cari user dari nomor kartu RFID.
pub async fn find_user_by_card(pool: &Pool, card: i64) -> Result<Option<(i64, String)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id, full_name FROM users WHERE rfid_cards = $1 AND is_active = TRUE",
            &[&card],
        )
        .await?;
    Ok(row.map(|r| (r.get(0), r.get(1))))
}

pub struct ProfilRow {
    pub full_name: String,
    pub username: Option<String>,
    pub role: String,
    pub email: Option<String>,
    pub phone_number: Option<String>,
    pub address: Option<String>,
    pub nis: Option<String>,
    pub points: i32,
    pub campus: Option<String>,
    pub major: Option<String>,
    pub gender: Option<String>,
    pub entry_year: Option<i16>,
}

/// Data profil lengkap satu user.
pub async fn profil_row(pool: &Pool, user_id: i64) -> Result<Option<ProfilRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT full_name, username, role, email, phone_number, address, nis, points, \
                    campus, major, gender, entry_year \
             FROM users WHERE id = $1",
            &[&user_id],
        )
        .await
        .context("profil_row")?;
    Ok(row.map(|r| ProfilRow {
        full_name: r.get(0),
        username: r.get(1),
        role: r.get(2),
        email: r.get(3),
        phone_number: r.get(4),
        address: r.get(5),
        nis: r.get(6),
        points: r.get(7),
        campus: r.get(8),
        major: r.get(9),
        gender: r.get(10),
        entry_year: r.get(11),
    }))
}

/// Ubah profil mahasiswa (kampus/jurusan/gender/tahun masuk) — santri sendiri
/// (migrasi 26 + 39).
pub async fn update_profile_extra(
    pool: &Pool,
    user_id: i64,
    campus: Option<&str>,
    major: Option<&str>,
    gender: Option<&str>,
    entry_year: Option<i16>,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE users SET campus = $2, major = $3, gender = $4, entry_year = $5 WHERE id = $1",
        &[&user_id, &campus, &major, &gender, &entry_year],
    )
    .await
    .context("update_profile_extra")?;
    Ok(())
}

/// Ubah kontak (email + alamat) user yang sedang login — semua peran. Email
/// UNIK di DB; benturan → error khusus agar service bisa pesan ramah.
pub async fn update_contact(
    pool: &Pool,
    user_id: i64,
    email: Option<&str>,
    address: Option<&str>,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE users SET email = $2, address = $3 WHERE id = $1",
        &[&user_id, &email, &address],
    )
    .await
    .map_err(|e| {
        // 23505 = unique_violation (email sudah dipakai user lain).
        if e.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
            anyhow::anyhow!("Email sudah dipakai akun lain.")
        } else {
            anyhow::Error::new(e).context("update_contact")
        }
    })?;
    Ok(())
}

/// Riwayat IPK satu santri (terbaru dulu). Return (id, semester, ipk).
pub async fn list_ipk(pool: &Pool, user_id: i64) -> Result<Vec<(i64, String, f64)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, semester, ipk FROM ipk_history WHERE user_id = $1 ORDER BY id DESC",
            &[&user_id],
        )
        .await
        .context("list_ipk")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1), r.get(2))).collect())
}

pub async fn add_ipk(pool: &Pool, user_id: i64, semester: &str, ipk: f64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO ipk_history (user_id, semester, ipk) VALUES ($1, $2, $3) RETURNING id",
            &[&user_id, &semester, &ipk],
        )
        .await
        .context("add_ipk")?;
    Ok(row.get(0))
}

/// Hapus entri IPK — hanya milik user (guard user_id).
pub async fn delete_ipk(pool: &Pool, user_id: i64, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM ipk_history WHERE id = $1 AND user_id = $2", &[&id, &user_id])
        .await
        .context("delete_ipk")?;
    Ok(n > 0)
}

/// Reset saldo poin SEMUA santri ke nilai awal semester (PRD: 300). Return
/// jumlah santri ter-reset.
///
/// PENTING: `users.points` dipegang trigger `trg_point_logs_balance` (migrasi
/// 32) — SATU-SATUNYA sumber kebenarannya adalah SUM(point_logs.delta). Karena
/// itu reset TIDAK menulis `users.points` langsung (dulu begitu, dan bikin
/// saldo menyimpang dari jumlah log: sekali ada log lama dihapus, trigger
/// mengurangi delta-nya dari angka yang sudah di-reset sembarang → hasil salah).
///
/// Gantinya: catat SATU log penyeimbang per santri sebesar selisih ke nilai
/// target, lalu biarkan trigger yang memindahkan saldo. Log-nya jadi bermakna
/// (delta = perubahan sebenarnya, bukan 0) dan jejak reset terlihat di riwayat.
pub async fn reset_semester_points(pool: &Pool, start: i32) -> Result<i64> {
    let c = pool.get().await?;
    // Hanya santri yang saldonya BEDA dari target yang dicatat — yang sudah pas
    // tak perlu log kosong. Trigger memproses per baris, jadi saldo akhir = $1.
    c.execute(
        "INSERT INTO point_logs (user_id, delta, reason, category) \
         SELECT id, $1 - points, 'Reset saldo poin awal semester', 'other' \
           FROM users \
          WHERE role IN ('santri', 'santri_finance') AND points <> $1",
        &[&start],
    )
    .await
    .context("reset_semester_points")?;
    // Jumlah santri terdampak = semua santri (yang sudah pas pun kini bernilai
    // target), supaya angka yang dilaporkan ke admin tetap bermakna.
    let row = c
        .query_one("SELECT COUNT(*) FROM users WHERE role IN ('santri', 'santri_finance')", &[])
        .await
        .context("reset_semester_points count")?;
    Ok(row.get(0))
}

/// Satu santri yang saldonya MELESET dari jumlah log poinnya.
pub struct SaldoMeleset {
    pub user_id: i64,
    pub full_name: String,
    /// Nilai di `users.points` sekarang.
    pub saldo: i32,
    /// Nilai yang seharusnya: `COALESCE(SUM(point_logs.delta), 0)`.
    pub seharusnya: i64,
}

/// Cari saldo poin yang MENYIMPANG dari akumulasi `point_logs`.
///
/// `users.points` adalah kolom TURUNAN yang dijaga trigger
/// `trg_point_logs_balance` (migrasi 32). Selama semua jalur hanya menulis
/// `point_logs`, keduanya mustahil berbeda — tapi "selama" itulah masalahnya.
/// Penyimpangan seperti itu tak menimbulkan galat, tak muncul di layar mana
/// pun, dan baru ketahuan kalau ada yang kebetulan menjumlahkan riwayat
/// seorang santri dengan tangan.
///
/// ⚠️ PRASYARAT: MIGRASI 72. Sampai migrasi itu dijalankan, fungsi ini melapor
/// SETIAP santri — dan laporannya benar tapi tak berguna. Sebabnya migrasi 28
/// menyetel `users.points DEFAULT 300`, sehingga saldo awal tiap santri masuk
/// lewat default kolom tanpa baris `point_logs` sama sekali; invarian
/// `points = ΣΔ` memang tak pernah berlaku untuk siapa pun. Migrasi 72
/// memasukkan saldo awal itu ke buku besar dan mengembalikan default ke 0,
/// setelah itu barulah selisih di sini benar-benar berarti "ada yang salah".
///
/// Fungsi ini TIDAK memperbaiki apa pun. Menambal otomatis akan menyembunyikan
/// jalur bocor yang menyebabkannya, dan yang perlu diperbaiki adalah jalur itu,
/// bukan angkanya. (Lagi pula menambal lewat penyisipan log MUSTAHIL: trigger
/// menggerakkan kedua sisi sebesar delta yang sama, jadi selisihnya kebal —
/// lihat uraian di migrasi 72.)
///
/// Santri tanpa satu pun log ikut diperiksa (`LEFT JOIN` + `COALESCE`): saldo
/// bukan-nol tanpa riwayat justru bentuk penyimpangan yang paling mencurigakan.
pub async fn saldo_menyimpang(pool: &Pool) -> Result<Vec<SaldoMeleset>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.points, COALESCE(SUM(pl.delta), 0)::bigint \
               FROM users u \
               LEFT JOIN point_logs pl ON pl.user_id = u.id \
              WHERE u.role IN ('santri', 'santri_finance') \
              GROUP BY u.id, u.full_name, u.points \
             HAVING u.points <> COALESCE(SUM(pl.delta), 0) \
              ORDER BY abs(u.points - COALESCE(SUM(pl.delta), 0)) DESC",
            &[],
        )
        .await
        .context("saldo_menyimpang")?;
    Ok(rows
        .into_iter()
        .map(|r| SaldoMeleset {
            user_id: r.get(0),
            full_name: r.get(1),
            saldo: r.get(2),
            seharusnya: r.get(3),
        })
        .collect())
}

/// Jumlah user (dipakai bootstrap seed).
pub async fn count_users(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c.query_one("SELECT COUNT(*) FROM users", &[]).await?;
    Ok(row.get(0))
}

/// Buat user admin awal (bootstrap saat tabel kosong).
pub async fn insert_admin(pool: &Pool, hash: &str) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO users (username, email, full_name, password_hash, role) \
         VALUES ('admin', 'admin@ppmafm.sch.id', 'Administrator', $1, 'admin')",
        &[&hash],
    )
    .await
    .context("insert_admin")?;
    Ok(())
}

// ── User Control (admin, migrasi 17) ─────────────────────────────────────────

pub struct UserListRow {
    pub id: i64,
    pub full_name: String,
    pub role: String,
    pub email: Option<String>,
    pub username: Option<String>,
    pub nis: Option<String>,
    pub is_active: bool,
}

/// Daftar user, opsional difilter per peran. Terurut peran lalu nama.
pub async fn list_users(pool: &Pool, role_filter: Option<&str>, limit: i64) -> Result<Vec<UserListRow>> {
    let c = pool.get().await?;
    let rows = match role_filter {
        Some(r) => {
            c.query(
                "SELECT id, full_name, role, email, username, nis, is_active FROM users \
                 WHERE role = $1 ORDER BY full_name LIMIT $2",
                &[&r, &limit],
            )
            .await
        }
        None => {
            c.query(
                "SELECT id, full_name, role, email, username, nis, is_active FROM users \
                 ORDER BY role, full_name LIMIT $1",
                &[&limit],
            )
            .await
        }
    }
    .context("list_users")?;
    Ok(rows
        .into_iter()
        .map(|r| UserListRow {
            id: r.get(0),
            full_name: r.get(1),
            role: r.get(2),
            email: r.get(3),
            username: r.get(4),
            nis: r.get(5),
            is_active: r.get(6),
        })
        .collect())
}

/// (total, santri, staff [guru+dewan_guru+pamong], nonaktif).
pub async fn user_counts(pool: &Pool) -> Result<(i64, i64, i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*), \
                COUNT(*) FILTER (WHERE role IN ('santri', 'santri_finance')), \
                COUNT(*) FILTER (WHERE role IN ('teacher','dewan_guru','supervisor')), \
                COUNT(*) FILTER (WHERE NOT is_active) \
             FROM users",
            &[],
        )
        .await
        .context("user_counts")?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub async fn set_active(pool: &Pool, user_id: i64, active: bool) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET is_active = $2, updated_at = NOW() WHERE id = $1",
            &[&user_id, &active],
        )
        .await
        .context("set_active")?;
    Ok(n > 0)
}

// Roles yang valid di sistem. SYNC dengan migration 44 database constraint.
const VALID_ROLES: &[&str] =
    &["admin", "ketua", "dewan_guru", "supervisor", "santri", "santri_finance", "parent"];

// ── Registrasi via link undangan (migrasi 19) ───────────────────────────────

/// Cek nomor HP sudah terdaftar atau belum (guard duplikat saat registrasi).
pub async fn find_by_phone(pool: &Pool, phone: &str) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT id FROM users WHERE phone_number = $1", &[&phone])
        .await
        .context("find_by_phone")?;
    Ok(row.map(|r| r.get(0)))
}

/// Ambil hash password user (untuk verifikasi sandi lama saat ganti sandi).
pub async fn get_password_hash(pool: &Pool, user_id: i64) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT password_hash FROM users WHERE id = $1", &[&user_id])
        .await
        .context("get_password_hash")?;
    Ok(row.map(|r| r.get(0)))
}

/// Ganti password (forgot-password via WA). Return true bila ada baris terubah.
pub async fn set_password_hash(pool: &Pool, user_id: i64, hash: &str) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET password_hash = $2 WHERE id = $1",
            &[&user_id, &hash],
        )
        .await
        .context("set_password_hash")?;
    Ok(n > 0)
}

/// Buat user dari alur registrasi. NIS/username/email tetap diisi admin
/// belakangan lewat /students atau /kontrol-pengguna.
///
/// `gender`/`campus`/`major`/`entry_year` hanya terisi untuk peran santri
/// (migrasi 47) — peran lain mengirim None. `entry_year` = tahun masuk PPM.
#[allow(clippy::too_many_arguments)]
pub async fn insert_registered_user(
    pool: &Pool,
    name: &str,
    phone: &str,
    role: &str,
    password_hash: &str,
    gender: Option<&str>,
    campus: Option<&str>,
    major: Option<&str>,
    entry_year: Option<i16>,
) -> Result<i64> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("insert_registered_user tx")?;
    let row = tx
        .query_one(
            "INSERT INTO users \
                (full_name, phone_number, role, password_hash, gender, campus, major, entry_year, \
                 points) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 0) RETURNING id",
            &[&name, &phone, &role, &password_hash, &gender, &campus, &major, &entry_year],
        )
        .await
        .context("insert_registered_user")?;
    let id: i64 = row.get(0);

    // ── SALDO AWAL LEWAT BUKU BESAR, BUKAN DEFAULT KOLOM ─────────────────────
    //
    // `points` diisi 0 secara eksplisit lalu saldo awalnya dicatat sebagai satu
    // baris `point_logs`; trigger `trg_point_logs_balance` (migrasi 32) yang
    // menaikkan saldonya. Hasil akhirnya sama — 300 untuk santri — tapi kali
    // ini ADA barisnya.
    //
    // Sebelumnya saldo awal datang dari DEFAULT kolom (migrasi 28), jadi 300
    // itu muncul di saldo tanpa jejak apa pun di riwayat. Dua akibatnya:
    //   • Riwayat poin seorang santri TAK BISA menjelaskan saldonya sendiri.
    //     Santri dengan saldo 210 melihat daftar log yang jumlahnya −90, dan
    //     tak ada baris mana pun yang menerangkan selisihnya.
    //   • `users.points = SUM(point_logs.delta)` — invarian yang dijaga trigger
    //     migrasi 32 dan diperiksa `saldo_menyimpang` — TIDAK PERNAH benar
    //     untuk siapa pun, sehingga pemeriksaannya melaporkan setiap santri.
    //
    // Perhatikan: drift TAK BISA diperbaiki dengan menyisipkan log penyeimbang.
    // Trigger menggerakkan KEDUA sisi sebesar delta yang sama, jadi selisih
    // (points − ΣΔ) kebal terhadap penyisipan. Satu-satunya cara menutupnya
    // adalah tidak membuka selisih itu sejak awal — yang dilakukan di sini.
    // (Migrasi 71 menutup selisih yang telanjur ada, dengan trigger dimatikan
    // sementara.)
    let saldo_awal = if crate::models::needs_student_profile(role) {
        crate::models::SEMESTER_START_POINTS
    } else {
        // Peran non-santri tak punya saldo poin — jangan buat baris kosong.
        0
    };
    if saldo_awal != 0 {
        tx.execute(
            "INSERT INTO point_logs (user_id, delta, reason, category) \
             VALUES ($1, $2, 'Saldo awal santri', 'other')",
            &[&id, &saldo_awal],
        )
        .await
        .context("insert_registered_user saldo awal")?;
    }
    tx.commit().await.context("insert_registered_user commit")?;
    Ok(id)
}

pub async fn set_role(pool: &Pool, user_id: i64, role: &str) -> Result<bool> {
    if !VALID_ROLES.contains(&role) {
        return Ok(false);
    }
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET role = $2, updated_at = NOW() WHERE id = $1",
            &[&user_id, &role],
        )
        .await
        .context("set_role")?;
    Ok(n > 0)
}

/// Pasang/lepas kartu RFID pengguna. `card` None = lepas.
///
/// Kolom `rfid_cards` UNIQUE — bentrok dipetakan ke pesan yang bisa dibaca
/// admin, bukan galat constraint mentah.
pub async fn set_rfid_card(pool: &Pool, user_id: i64, card: Option<i64>) -> Result<()> {
    let c = pool.get().await?;
    c.execute("UPDATE users SET rfid_cards = $2 WHERE id = $1", &[&user_id, &card])
        .await
        .map_err(|e| {
            if e.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                anyhow::anyhow!("Kartu ini sudah dipakai pengguna lain.")
            } else {
                anyhow::Error::new(e).context("set_rfid_card")
            }
        })?;
    Ok(())
}

pub struct UserPickRow {
    pub id: i64,
    pub full_name: String,
    pub role: String,
    pub nis: Option<String>,
    pub rfid_cards: Option<i64>,
}

/// Cari pengguna AKTIF untuk pemasangan kartu — semua peran (pamong & dewan
/// guru juga menempel kartu di gerbang, bukan santri saja). Cocokkan nama, NIS,
/// atau nomor HP.
pub async fn search_users_for_card(pool: &Pool, q: &str, limit: i64) -> Result<Vec<UserPickRow>> {
    let c = pool.get().await?;
    let pattern = format!("%{}%", q.trim());
    let rows = c
        .query(
            "SELECT id, full_name, role, nis, rfid_cards FROM users \
             WHERE is_active = TRUE \
               AND (full_name ILIKE $1 OR nis ILIKE $1 OR phone_number ILIKE $1) \
             ORDER BY full_name LIMIT $2",
            &[&pattern, &limit],
        )
        .await
        .context("search_users_for_card")?;
    Ok(rows
        .into_iter()
        .map(|r| UserPickRow {
            id: r.get(0),
            full_name: r.get(1),
            role: r.get(2),
            nis: r.get(3),
            rfid_cards: r.get(4),
        })
        .collect())
}

// ── Manajemen User (/manajemen-user, admin & ketua) ──────────────────────────

/// Daftar user untuk halaman manajemen: saring status aktif, peran, dan kata
/// kunci (nama / NIS / nomor HP).
///
/// `aktif`: `Some(true)` hanya yang aktif, `Some(false)` hanya yang nonaktif,
/// `None` semuanya. Halaman ini ADA justru karena yang nonaktif tak muncul di
/// mana pun lagi — seluruh aplikasi menyaring `is_active = TRUE`, sebagaimana
/// mestinya, sehingga 512 santri hasil impor tak terlihat sampai diaktifkan.
///
/// Pencariannya `ILIKE` tanpa index khusus: daftarnya ribuan baris, bukan
/// jutaan, dan menambah index trigram untuk itu berarti menanggung biaya tulis
/// di setiap perubahan user demi satu layar yang dibuka pengelola sesekali.
pub async fn list_users_managed(
    pool: &Pool,
    aktif: Option<bool>,
    role_filter: Option<&str>,
    angkatan: Option<i16>,
    cari: &str,
    limit: i64,
) -> Result<Vec<crate::models::ManagedUser>> {
    let c = pool.get().await?;
    // `$n IS NULL OR …` — satu pernyataan untuk semua kombinasi filter, jadi
    // tak ada empat varian SQL yang harus dijaga tetap sepakat.
    let pola = if cari.trim().is_empty() {
        String::new()
    } else {
        format!("%{}%", cari.trim())
    };
    let pola_opt = (!pola.is_empty()).then_some(pola);
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.role, u.is_active, u.nis, u.phone_number, \
                    u.entry_year, u.gender, u.campus, u.major, \
                    u.mubalegh_status, u.pendidikan_status, u.points, \
                    EXISTS (SELECT 1 FROM point_logs pl WHERE pl.user_id = u.id) \
               FROM users u \
              WHERE ($1::bool IS NULL OR u.is_active = $1) \
                AND ($2::text IS NULL OR u.role = $2) \
                AND ($3::text IS NULL OR u.full_name ILIKE $3 \
                     OR COALESCE(u.nis, '') ILIKE $3 \
                     OR COALESCE(u.phone_number, '') ILIKE $3) \
                AND ($4::int2 IS NULL OR u.entry_year = $4) \
              ORDER BY u.is_active DESC, u.full_name \
              LIMIT $5",
            &[&aktif, &role_filter, &pola_opt, &angkatan, &limit],
        )
        .await
        .context("list_users_managed")?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let role: String = r.get(2);
            crate::models::ManagedUser {
                id: r.get(0),
                full_name: r.get(1),
                role_label: crate::models::role_label(&role).to_string(),
                role,
                is_active: r.get(3),
                nis: r.get(4),
                phone_number: r.get(5),
                entry_year: r.get(6),
                gender: r.get(7),
                campus: r.get(8),
                major: r.get(9),
                mubalegh_status: r.get(10),
                pendidikan_status: r.get(11),
                points: r.get(12),
                has_point_logs: r.get(13),
            }
        })
        .collect())
}

/// Tahun angkatan yang BENAR-BENAR ada di data, terbaru dulu.
///
/// Diambil dari database, bukan rentang tahun yang dikarang di klien: daftar
/// induk pondok ini membentang 2010–2025 dan akan terus bertambah, sedangkan
/// rentang tetap di kode pasti basi tanpa ada yang menyadarinya — dan menawarkan
/// tahun yang tak berpenghuni hanya menghasilkan hasil kosong yang
/// membingungkan.
pub async fn angkatan_tersedia(pool: &Pool) -> Result<Vec<i16>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT entry_year FROM users \
              WHERE entry_year IS NOT NULL ORDER BY entry_year DESC",
            &[],
        )
        .await
        .context("angkatan_tersedia")?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}

/// Aktifkan user; santri yang BELUM PUNYA catatan poin sekalian diberi saldo
/// awal. Return `(berhasil, saldo_diberikan)`.
///
/// ── KENAPA SALDO AWAL DITENTUKAN DARI ADA-TIDAKNYA LOG ───────────────────────
/// Dua hal berbeda sama-sama berakhir di sini:
///   • santri dari daftar induk yang baru pertama kali diaktifkan — ia memang
///     harus mulai dari 300;
///   • santri yang sempat dinonaktifkan lalu diaktifkan lagi — saldonya masih
///     tersimpan, dan memberinya 300 lagi berarti menghadiahi kepergiannya.
/// Yang membedakan keduanya bukan status aktifnya, melainkan apakah ia sudah
/// punya riwayat poin sama sekali.
///
/// Saldonya ditulis sebagai baris `point_logs`, bukan dengan menyetel
/// `users.points`: trigger `trg_point_logs_balance` (migrasi 32) yang
/// memindahkan angkanya, sehingga `points = SUM(delta)` tetap benar dan yang
/// bersangkutan tak muncul di laporan rekonsiliasi saldo.
pub async fn activate_user(pool: &Pool, user_id: i64, saldo_awal: i32) -> Result<(bool, bool)> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("activate_user tx")?;
    let n = tx
        .execute(
            "UPDATE users SET is_active = TRUE, updated_at = NOW() \
             WHERE id = $1 AND is_active = FALSE",
            &[&user_id],
        )
        .await
        .context("activate_user")?;
    if n == 0 {
        tx.rollback().await.ok();
        return Ok((false, false));
    }
    // Hanya santri, dan hanya yang benar-benar belum punya riwayat poin.
    let row = tx
        .query_opt(
            "INSERT INTO point_logs (user_id, delta, reason, category) \
             SELECT u.id, $2, 'Saldo awal santri', 'other' \
               FROM users u \
              WHERE u.id = $1 \
                AND u.role IN ('santri', 'santri_finance') \
                AND NOT EXISTS (SELECT 1 FROM point_logs pl WHERE pl.user_id = u.id) \
             RETURNING 1",
            &[&user_id, &saldo_awal],
        )
        .await
        .context("activate_user saldo awal")?;
    tx.commit().await.context("activate_user commit")?;
    Ok((true, row.is_some()))
}

/// Aktifkan BANYAK user sekaligus; santri yang belum punya catatan poin ikut
/// diberi saldo awal. Return `(jumlah_aktif, jumlah_dapat_saldo)`.
///
/// Set-based, bukan memanggil `activate_user` dalam loop: mengaktifkan satu
/// angkatan berarti puluhan user, dan versi loop menghasilkan puluhan
/// transaksi yang bisa putus di tengah — sebagian aktif, sebagian tidak, tanpa
/// ada yang tahu di mana berhentinya. Di sini keduanya satu transaksi.
///
/// Syarat saldo awal SAMA dengan versi satuannya: "belum punya point_logs",
/// bukan "belum aktif" — lihat [`activate_user`] untuk alasannya.
pub async fn activate_users(
    pool: &Pool,
    user_ids: &[i64],
    saldo_awal: i32,
) -> Result<(i64, i64)> {
    if user_ids.is_empty() {
        return Ok((0, 0));
    }
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("activate_users tx")?;
    let rows = tx
        .query(
            "UPDATE users SET is_active = TRUE, updated_at = NOW() \
              WHERE id = ANY($1::bigint[]) AND is_active = FALSE \
             RETURNING id",
            &[&user_ids],
        )
        .await
        .context("activate_users")?;
    let aktif: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
    if aktif.is_empty() {
        tx.rollback().await.ok();
        return Ok((0, 0));
    }
    let saldo = tx
        .execute(
            "INSERT INTO point_logs (user_id, delta, reason, category) \
             SELECT u.id, $2, 'Saldo awal santri', 'other' \
               FROM users u \
              WHERE u.id = ANY($1::bigint[]) \
                AND u.role IN ('santri', 'santri_finance') \
                AND NOT EXISTS (SELECT 1 FROM point_logs pl WHERE pl.user_id = u.id)",
            &[&aktif, &saldo_awal],
        )
        .await
        .context("activate_users saldo awal")?;
    tx.commit().await.context("activate_users commit")?;
    Ok((aktif.len() as i64, saldo as i64))
}

/// Nonaktifkan BANYAK user sekaligus. Return jumlah yang berubah.
pub async fn deactivate_users(pool: &Pool, user_ids: &[i64]) -> Result<i64> {
    if user_ids.is_empty() {
        return Ok(0);
    }
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET is_active = FALSE, updated_at = NOW() \
              WHERE id = ANY($1::bigint[]) AND is_active = TRUE",
            &[&user_ids],
        )
        .await
        .context("deactivate_users")?;
    Ok(n as i64)
}

/// Ubah detail profil satu user (halaman manajemen).
///
/// Kolom teks kosong disimpan sebagai NULL, bukan string kosong: `nis` dan
/// `phone_number` UNIK, dan dua baris ber-string-kosong akan bertabrakan
/// padahal maksudnya sama-sama "belum diisi". NULL tak pernah bertabrakan.
pub async fn update_user_profile(
    pool: &Pool,
    user_id: i64,
    p: &crate::models::ProfilEdit,
) -> Result<bool> {
    let kosong_jadi_null = |s: &str| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    let nama = p.full_name.trim();
    if nama.is_empty() {
        anyhow::bail!("Nama tidak boleh kosong.");
    }
    let nis = kosong_jadi_null(&p.nis);
    let hp = kosong_jadi_null(&p.phone_number);
    let gender = kosong_jadi_null(&p.gender);
    let campus = kosong_jadi_null(&p.campus);
    let major = kosong_jadi_null(&p.major);
    let mub = kosong_jadi_null(&p.mubalegh_status);
    let pen = kosong_jadi_null(&p.pendidikan_status);

    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE users SET full_name = $2, nis = $3, phone_number = $4, \
                    entry_year = $5, gender = $6, campus = $7, major = $8, \
                    mubalegh_status = $9, pendidikan_status = $10, updated_at = NOW() \
             WHERE id = $1",
            &[
                &user_id, &nama, &nis, &hp, &p.entry_year, &gender, &campus, &major, &mub, &pen,
            ],
        )
        .await
        .map_err(|e| {
            // NIS/HP unik: sampaikan sebagai kalimat, bukan galat Postgres mentah.
            if e.code() == Some(&tokio_postgres::error::SqlState::UNIQUE_VIOLATION) {
                let apa = e
                    .as_db_error()
                    .and_then(|d| d.constraint())
                    .map(|c| if c.contains("phone") { "Nomor HP" } else { "NIS" })
                    .unwrap_or("NIS/Nomor HP");
                anyhow::anyhow!("{apa} itu sudah dipakai user lain.")
            } else {
                anyhow::Error::new(e).context("update_user_profile")
            }
        })?;
    Ok(n > 0)
}
