//! repository/kelas.rs — Query agregat sisi STAF/GURU/DEWAN GURU: dashboard
//! staf, ranking kelas, insight guru, papan poin santri.
//!
//! Semua query di sini SCOPED lewat parameter `teacher_id: Option<i64>`:
//! `None` = seluruh pesantren (admin/dewan guru), `Some(id)` = hanya
//! kelas-kelas yang sesi terakhirnya diampu guru tsb (role teacher biasa).

use anyhow::{Context, Result};
use deadpool_postgres::Pool;

// ── Dashboard staf ───────────────────────────────────────────────────────────────

/// (total_santri, santri_baru_bulan_ini, hadir_hari_ini, izin_pending).
pub async fn staf_stats(pool: &Pool) -> Result<(i64, i64, i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT \
                (SELECT COUNT(*) FROM users WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE), \
                (SELECT COUNT(*) FROM users WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE \
                    AND created_at >= (date_trunc('month', NOW() AT TIME ZONE 'Asia/Jakarta') AT TIME ZONE 'Asia/Jakarta')), \
                (SELECT COUNT(DISTINCT a.user_id) FROM attendances a JOIN users u ON u.id = a.user_id \
                    WHERE u.role IN ('santri', 'santri_finance') AND a.status IN ('present','late') \
                    AND a.scan_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date), \
                (SELECT COUNT(*) FROM permit_requests p \
                    LEFT JOIN classes tc ON tc.id = p.class_id \
LEFT JOIN LATERAL ( \
                        SELECT c.* FROM class_participants cp_ku \
                          JOIN classes c ON c.id = cp_ku.class_id \
                         WHERE cp_ku.user_id = p.user_id \
                         ORDER BY (c.category = 'kbm') DESC, c.id \
                         LIMIT 1 \
                    ) cl ON TRUE \
                    WHERE p.guru_status = 'pending' AND p.pamong_status <> 'rejected' \
                      AND CASE WHEN COALESCE(tc.require_pamong, cl.require_pamong, TRUE) \
                                    AND COALESCE(tc.pamong_id, cl.pamong_id) IS NOT NULL \
                               THEN p.pamong_status = 'approved' ELSE TRUE END)",
            &[],
        )
        .await
        .context("staf_stats")?;
    Ok((row.get(0), row.get(1), row.get(2), row.get(3)))
}

pub struct LiveSesiRow {
    pub id: i64,
    pub title: String,
    pub teacher: String,
    pub santri_count: i64,
    pub state: String,
    pub time_label: Option<chrono::NaiveTime>,
    /// Jam mulainya SUDAH LEWAT (menurut jam WIB sekarang).
    ///
    /// Dihitung di SQL, bukan di Rust atau di browser: perbandingannya harus
    /// memakai jam WIB, dan query ini sudah memakai `AT TIME ZONE
    /// 'Asia/Jakarta'` untuk memilih sesi hari ini — memakai jam browser akan
    /// salah bagi pengguna di zona waktu lain.
    ///
    /// Perlu karena `status` sesi TIDAK bergerak sendiri: sesi subuh tetap
    /// `scheduled` sampai malam, jadi "sudah lewat atau belum" tak bisa
    /// disimpulkan dari status saja.
    pub past: bool,
    /// Jam WIB sekarang berada DI ANTARA jam mulai dan jam selesai.
    ///
    /// Alasannya sama dengan `past`: karena `status` tak pernah berubah jadi
    /// `ongoing` dengan sendirinya, sesi yang betul-betul sedang berlangsung
    /// tetap terbaca `scheduled` dan tampil sebagai "jadwal berikutnya" —
    /// padahal jamnya sedang berjalan.
    pub ongoing: bool,
}

/// Sesi kelas hari ini (berlangsung + akan datang), untuk kartu "Sesi Live".
pub async fn today_sessions(pool: &Pool, limit: i64) -> Result<Vec<LiveSesiRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT s.id, COALESCE(s.title, cs.title, c.name), COALESCE(t.full_name, 'Belum ditentukan'), \
                    (SELECT COUNT(*) FROM class_participants cp WHERE cp.class_id = c.id), \
                    s.status, cs.start_time, \
                    COALESCE(cs.end_time, cs.start_time) < (NOW() AT TIME ZONE 'Asia/Jakarta')::time \
                        AS past, \
                    (cs.start_time IS NOT NULL AND cs.end_time IS NOT NULL \
                     AND (NOW() AT TIME ZONE 'Asia/Jakarta')::time \
                            BETWEEN cs.start_time AND cs.end_time) AS ongoing \
             FROM class_sessions s \
             JOIN classes c ON c.id = s.class_id \
             LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
             LEFT JOIN users t ON t.id = s.teacher_id \
             WHERE s.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date \
             ORDER BY CASE s.status WHEN 'ongoing' THEN 0 WHEN 'scheduled' THEN 1 ELSE 2 END, \
                      cs.start_time ASC NULLS LAST \
             LIMIT $1",
            &[&limit],
        )
        .await
        .context("today_sessions")?;
    Ok(rows
        .into_iter()
        .map(|r| LiveSesiRow {
            id: r.get(0),
            title: r.get(1),
            teacher: r.get(2),
            santri_count: r.get(3),
            state: r.get(4),
            time_label: r.get(5),
            // NULL (jadwal tanpa jam) → anggap belum lewat: lebih baik kartunya
            // tetap tampil daripada jadwal tanpa jam hilang diam-diam.
            past: r.get::<_, Option<bool>>(6).unwrap_or(false),
            ongoing: r.get::<_, Option<bool>>(7).unwrap_or(false),
        })
        .collect())
}

pub struct LatestAttRow {
    pub name: String,
    pub class_name: Option<String>,
    pub scanned_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

/// Kehadiran terbaru (semua santri) — untuk tabel "Kehadiran Terbaru" staf.
pub async fn latest_attendance(pool: &Pool, limit: i64) -> Result<Vec<LatestAttRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, COALESCE(cs.title, c.name), a.scanned_at, a.status \
             FROM attendances a \
             JOIN users u ON u.id = a.user_id \
             LEFT JOIN class_schedules cs ON cs.id = a.class_schedule_id \
             LEFT JOIN classes c ON c.id = cs.class_id \
             WHERE u.role IN ('santri', 'santri_finance') \
             ORDER BY a.scanned_at DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("latest_attendance")?;
    Ok(rows
        .into_iter()
        .map(|r| LatestAttRow {
            name: r.get(0),
            class_name: r.get(1),
            scanned_at: r.get(2),
            status: r.get(3),
        })
        .collect())
}

// ── Analisis (guru / dewan guru) ─────────────────────────────────────────────────

/// (pct_kehadiran, rata2_poin, sesi_terverifikasi) — dalam cakupan 30 hari
/// terakhir. `teacher_id = None` → seluruh pesantren.
pub async fn analisis_summary(pool: &Pool, teacher_id: Option<i64>) -> Result<(i32, i32, i64)> {
    let c = pool.get().await?;
    let row = match teacher_id {
        None => {
            c.query_one(
                "SELECT \
                    COALESCE(ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(*), 0)), 0)::INT, \
                    COALESCE((SELECT ROUND(AVG(points)) FROM users WHERE role IN ('santri', 'santri_finance')), 0)::INT, \
                    (SELECT COUNT(*) FROM attendances WHERE pamong_status = 'approved' \
                        AND pamong_at >= NOW() - INTERVAL '30 days') \
                 FROM attendances a JOIN users u ON u.id = a.user_id \
                 WHERE u.role IN ('santri', 'santri_finance') AND a.scanned_at >= NOW() - INTERVAL '30 days'",
                &[],
            )
            .await
        }
        Some(tid) => {
            c.query_one(
                "SELECT \
                    COALESCE(ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(*), 0)), 0)::INT, \
                    COALESCE((SELECT ROUND(AVG(u2.points)) FROM users u2 \
                        JOIN class_participants cp ON cp.user_id = u2.id \
                        WHERE u2.role IN ('santri', 'santri_finance') AND cp.class_id IN \
                            (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1)), 0)::INT, \
                    (SELECT COUNT(*) FROM attendances a2 \
                        JOIN class_sessions s2 ON s2.id = a2.class_session_id \
                        WHERE s2.teacher_id = $1 AND a2.pamong_status = 'approved' \
                        AND a2.pamong_at >= NOW() - INTERVAL '30 days') \
                 FROM attendances a \
                 JOIN class_sessions s ON s.id = a.class_session_id \
                 WHERE s.teacher_id = $1 AND a.scanned_at >= NOW() - INTERVAL '30 days'",
                &[&tid],
            )
            .await
        }
    }
    .context("analisis_summary")?;
    Ok((row.get(0), row.get(1), row.get(2)))
}

/// Tren kehadiran 7 hari terakhir (persentase per hari).
pub async fn attendance_trend_7d(
    pool: &Pool,
    teacher_id: Option<i64>,
) -> Result<Vec<(chrono::NaiveDate, i32)>> {
    let c = pool.get().await?;
    let rows = match teacher_id {
        None => {
            c.query(
                "SELECT d::date, COALESCE(( \
                    SELECT ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) / NULLIF(COUNT(*), 0)) \
                    FROM attendances a JOIN users u ON u.id = a.user_id \
                    WHERE u.role IN ('santri', 'santri_finance') AND a.scan_date = d::date \
                 ), 0)::INT \
                 FROM generate_series((NOW() AT TIME ZONE 'Asia/Jakarta')::date - INTERVAL '6 days', \
                        (NOW() AT TIME ZONE 'Asia/Jakarta')::date, INTERVAL '1 day') d",
                &[],
            )
            .await
        }
        Some(tid) => {
            c.query(
                "SELECT d::date, COALESCE(( \
                    SELECT ROUND(100.0 * COUNT(*) FILTER (WHERE a.status IN ('present','late')) / NULLIF(COUNT(*), 0)) \
                    FROM attendances a JOIN class_sessions s ON s.id = a.class_session_id \
                    WHERE s.teacher_id = $1 AND a.scan_date = d::date \
                 ), 0)::INT \
                 FROM generate_series((NOW() AT TIME ZONE 'Asia/Jakarta')::date - INTERVAL '6 days', \
                        (NOW() AT TIME ZONE 'Asia/Jakarta')::date, INTERVAL '1 day') d",
                &[&tid],
            )
            .await
        }
    }
    .context("attendance_trend_7d")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

pub struct ClassRankRow {
    pub name: String,
    pub attendance_pct: i32,
    pub avg_points: i32,
    pub santri_count: i64,
}

/// Ranking kelas berdasar persentase kehadiran 30 hari terakhir.
pub async fn class_ranking(
    pool: &Pool,
    teacher_id: Option<i64>,
    limit: i64,
) -> Result<Vec<ClassRankRow>> {
    let c = pool.get().await?;
    let rows = match teacher_id {
        None => {
            c.query(
                "SELECT c.name, \
                    COALESCE(ROUND(100.0 * COUNT(a.*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(a.*), 0)), 0)::INT, \
                    COALESCE(ROUND(AVG(u.points)), 0)::INT, \
                    COUNT(DISTINCT cp.user_id) \
                 FROM classes c \
                 LEFT JOIN class_participants cp ON cp.class_id = c.id \
                 LEFT JOIN users u ON u.id = cp.user_id AND u.role IN ('santri', 'santri_finance') \
                 LEFT JOIN class_schedules cs ON cs.class_id = c.id \
                 LEFT JOIN attendances a ON a.class_schedule_id = cs.id \
                    AND a.scanned_at >= NOW() - INTERVAL '30 days' \
                 GROUP BY c.id, c.name ORDER BY 2 DESC LIMIT $1",
                &[&limit],
            )
            .await
        }
        Some(tid) => {
            c.query(
                "SELECT c.name, \
                    COALESCE(ROUND(100.0 * COUNT(a.*) FILTER (WHERE a.status IN ('present','late')) \
                        / NULLIF(COUNT(a.*), 0)), 0)::INT, \
                    COALESCE(ROUND(AVG(u.points)), 0)::INT, \
                    COUNT(DISTINCT cp.user_id) \
                 FROM classes c \
                 JOIN class_sessions s ON s.class_id = c.id AND s.teacher_id = $1 \
                 LEFT JOIN class_participants cp ON cp.class_id = c.id \
                 LEFT JOIN users u ON u.id = cp.user_id AND u.role IN ('santri', 'santri_finance') \
                 LEFT JOIN attendances a ON a.class_session_id = s.id \
                 GROUP BY c.id, c.name ORDER BY 2 DESC LIMIT $2",
                &[&tid, &limit],
            )
            .await
        }
    }
    .context("class_ranking")?;
    Ok(rows
        .into_iter()
        .map(|r| ClassRankRow {
            name: r.get(0),
            attendance_pct: r.get(1),
            avg_points: r.get(2),
            santri_count: r.get(3),
        })
        .collect())
}

pub struct TeacherInsightRow {
    pub name: String,
    pub sessions_count: i64,
    pub attendance_pct: i32,
}

/// Insight kinerja pengajar (dewan guru saja — semua guru).
pub async fn teacher_insight(pool: &Pool, limit: i64) -> Result<Vec<TeacherInsightRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT t.full_name, COUNT(DISTINCT s.id), \
                COALESCE(ROUND(100.0 * COUNT(a.*) FILTER (WHERE a.status IN ('present','late')) \
                    / NULLIF(COUNT(a.*), 0)), 0)::INT \
             FROM users t \
             JOIN class_sessions s ON s.teacher_id = t.id \
             LEFT JOIN attendances a ON a.class_session_id = s.id \
             WHERE t.role IN ('teacher', 'dewan_guru') \
               AND s.session_date >= CURRENT_DATE - INTERVAL '30 days' \
             GROUP BY t.id, t.full_name ORDER BY 3 DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("teacher_insight")?;
    Ok(rows
        .into_iter()
        .map(|r| TeacherInsightRow {
            name: r.get(0),
            sessions_count: r.get(1),
            attendance_pct: r.get(2),
        })
        .collect())
}

// ── Poin santri ───────────────────────────────────────────────────────────────────

pub struct PointRowDb {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub points: i32,
}

/// Papan poin santri, terurut (desc = tertinggi dulu).
///
/// Dua hal di query bawah sengaja dibuat DETERMINISTIK, dan keduanya dulu
/// tidak:
///   • Subquery nama kelas memakai `LIMIT 1` tanpa `ORDER BY` — santri yang
///     ikut lebih dari satu kelas bisa tampil dengan nama kelas berbeda-beda
///     tiap kali halaman dimuat, tergantung rencana query yang dipilih
///     Postgres. Sekarang diikat ke `class_id` terkecil.
///   • `ORDER BY u.points` tanpa pemecah seri — poin yang sama membuat urutan
///     antar-santri bebas, jadi papan bisa berganti susunan sendiri (dan dengan
///     `LIMIT`, santri di ambang batas bisa muncul-hilang). `u.id` dipakai
///     sebagai pemecah seri yang stabil.
pub async fn points_board(
    pool: &Pool,
    teacher_id: Option<i64>,
    limit: i64,
    desc: bool,
) -> Result<Vec<PointRowDb>> {
    let c = pool.get().await?;
    let order = if desc { "DESC" } else { "ASC" };
    let rows = match teacher_id {
        None => {
            let sql = format!(
                "SELECT u.id, u.full_name, u.nis, \
                    (SELECT c.name FROM class_participants cp JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1), \
                    u.points \
                 FROM users u WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
                 ORDER BY u.points {order}, u.id LIMIT $1"
            );
            c.query(&sql, &[&limit]).await
        }
        Some(tid) => {
            let sql = format!(
                "SELECT u.id, u.full_name, u.nis, \
                     (SELECT c2.name FROM class_participants cp2 \
                         JOIN classes c2 ON c2.id = cp2.class_id \
                         WHERE cp2.user_id = u.id \
                           AND cp2.class_id IN (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1) \
                         ORDER BY cp2.class_id LIMIT 1), \
                     u.points \
                 FROM users u \
                 WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
                   AND EXISTS ( \
                       SELECT 1 FROM class_participants cp WHERE cp.user_id = u.id \
                       AND cp.class_id IN (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1) \
                   ) \
                 ORDER BY u.points {order}, u.id LIMIT $2"
            );
            c.query(&sql, &[&tid, &limit]).await
        }
    }
    .context("points_board")?;
    Ok(rows
        .into_iter()
        .map(|r| PointRowDb {
            user_id: r.get(0),
            name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            points: r.get(4),
        })
        .collect())
}

/// Satu kelas yang diikuti santri, dengan jenjangnya (migrasi 16) —
/// "" bila kelas itu di luar sistem jenjang Bacaan/Makna.
pub struct StudentClassRow {
    pub jenjang: String,
    pub name: String,
}

pub struct StudentBoardRow {
    pub user_id: i64,
    pub name: String,
    pub nis: Option<String>,
    /// Tahun masuk PPM. Dibaca dari kolomnya, bukan ditebak dari NIS — NIS
    /// resmi pondok diawali "5000", bukan tahun.
    pub entry_year: Option<i16>,
    pub points: i32,
    /// SEMUA kelas yang diikuti santri (biasanya satu per jenjang — satu
    /// Bacaan + satu Makna) — beda dari `points_board` yang cuma ambil SATU
    /// kelas (LIMIT 1) krn dulu diasumsikan satu santri = satu kelas.
    pub classes: Vec<StudentClassRow>,
}

/// Papan santri UTUH (dipakai halaman Students) — tak seperti `points_board`,
/// mengambil SEMUA kelas tiap santri (bukan LIMIT 1) agar santri yang ikut
/// kelas Bacaan SEKALIGUS Makna tetap tampil keduanya di UI.
/// Jumlah santri aktif yang COCOK dengan penyaring — dipakai halaman Students
/// supaya angka yang dipajang bukan sekadar "berapa yang sudah termuat".
///
/// Halaman lama menulis "Total 300 santri terdaftar" padahal 300 itu batas
/// pengambilannya, bukan jumlah santri; pada pondok berisi 500, dua ratus
/// sisanya tak pernah disebut.
pub async fn count_students(pool: &Pool, q: &str, angkatan: Option<i16>) -> Result<i64> {
    let c = pool.get().await?;
    let qt = q.trim();
    let pola = format!("%{qt}%");
    let row = c
        .query_one(
            "SELECT COUNT(*) FROM users \
              WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE \
                AND ($1 = '' OR full_name ILIKE $2 OR COALESCE(nis, '') ILIKE $2) \
                AND ($3::int2 IS NULL OR entry_year = $3)",
            &[&qt, &pola, &angkatan],
        )
        .await
        .context("count_students")?;
    Ok(row.get(0))
}

/// Satu HALAMAN papan santri: saring nama/NIS + angkatan, lalu `LIMIT/OFFSET`.
///
/// Penyaringan pindah ke SERVER sejak daftarnya dipaginasi. Selama seluruh
/// santri termuat sekaligus, menyaring di klien memang cukup — tapi begitu
/// hanya sepotong yang ada di memori, filter klien cuma menyaring potongan itu
/// dan santri yang cocok di halaman berikutnya tak pernah muncul.
///
/// Urutannya `points DESC, full_name, id`: dua tie-breaker terakhir WAJIB ada.
/// Ratusan santri berbagi nilai poin yang sama (semuanya mulai dari 300), dan
/// `ORDER BY points DESC` saja tak menentukan urutan di antara mereka —
/// Postgres boleh mengembalikannya berbeda tiap query, sehingga baris yang
/// sama bisa muncul dua kali di halaman berbeda sementara yang lain terlewat.
pub async fn students_page(
    pool: &Pool,
    q: &str,
    angkatan: Option<i16>,
    limit: i64,
    offset: i64,
) -> Result<Vec<StudentBoardRow>> {
    let c = pool.get().await?;
    let qt = q.trim();
    let pola = format!("%{qt}%");
    let students = c
        .query(
            "SELECT id, full_name, nis, points, entry_year FROM users \
             WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE \
               AND ($1 = '' OR full_name ILIKE $2 OR COALESCE(nis, '') ILIKE $2) \
               AND ($3::int2 IS NULL OR entry_year = $3) \
             ORDER BY points DESC, full_name, id \
             LIMIT $4 OFFSET $5",
            &[&qt, &pola, &angkatan, &limit, &offset],
        )
        .await
        .context("students_with_classes: students")?;
    let ids: Vec<i64> = students.iter().map(|r| r.get(0)).collect();
    let class_rows = c
        .query(
            "SELECT DISTINCT cp.user_id, COALESCE(c.jenjang, ''), c.name \
             FROM class_participants cp JOIN classes c ON c.id = cp.class_id \
             WHERE cp.user_id = ANY($1) \
             ORDER BY 2 NULLS LAST, 3",
            &[&ids],
        )
        .await
        .context("students_with_classes: classes")?;
    let mut by_user: std::collections::HashMap<i64, Vec<StudentClassRow>> =
        std::collections::HashMap::new();
    for r in class_rows {
        let uid: i64 = r.get(0);
        by_user
            .entry(uid)
            .or_default()
            .push(StudentClassRow { jenjang: r.get(1), name: r.get(2) });
    }
    Ok(students
        .into_iter()
        .map(|r| {
            let user_id: i64 = r.get(0);
            StudentBoardRow {
                classes: by_user.remove(&user_id).unwrap_or_default(),
                user_id,
                name: r.get(1),
                nis: r.get(2),
                points: r.get(3),
                entry_year: r.get(4),
            }
        })
        .collect())
}

/// (rata-rata poin, jumlah santri) dalam cakupan.
pub async fn points_avg(pool: &Pool, teacher_id: Option<i64>) -> Result<(i32, i64)> {
    let c = pool.get().await?;
    let row = match teacher_id {
        None => {
            c.query_one(
                "SELECT COALESCE(ROUND(AVG(points)), 0)::INT, COUNT(*) \
                 FROM users WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE",
                &[],
            )
            .await
        }
        Some(tid) => {
            c.query_one(
                "SELECT COALESCE(ROUND(AVG(u.points)), 0)::INT, COUNT(DISTINCT u.id) \
                 FROM users u JOIN class_participants cp ON cp.user_id = u.id \
                 WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
                   AND cp.class_id IN (SELECT DISTINCT class_id FROM class_sessions WHERE teacher_id = $1)",
                &[&tid],
            )
            .await
        }
    }
    .context("points_avg")?;
    Ok((row.get(0), row.get(1)))
}

/// Total poin pelanggaran (delta negatif, kategori 'discipline') 30 hari
/// terakhir — kartu "Poin Pelanggaran Aktif" laporan institusi. Nilai POSITIF
/// (magnitude, bukan negatif) agar langsung enak ditampilkan.
pub async fn active_violation_points(pool: &Pool) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COALESCE(-SUM(delta), 0)::BIGINT FROM point_logs \
             WHERE delta < 0 AND category = 'discipline' AND created_at >= NOW() - INTERVAL '30 days'",
            &[],
        )
        .await
        .context("active_violation_points")?;
    Ok(row.get(0))
}

/// Baris riwayat poin TERBARU seluruh santri (nama+kelas) — "Ringkasan Poin"
/// laporan institusi.
pub async fn recent_points_all(pool: &Pool, limit: i64) -> Result<Vec<(String, String, String, i32)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, COALESCE(c.name, '-'), p.reason, p.delta \
             FROM point_logs p \
             JOIN users u ON u.id = p.user_id \
             LEFT JOIN classes c ON c.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE u.role IN ('santri', 'santri_finance') \
             ORDER BY p.created_at DESC LIMIT $1",
            &[&limit],
        )
        .await
        .context("recent_points_all")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))).collect())
}

/// Riwayat poin SATU santri, dipisah prestasi (delta>0) / pelanggaran (delta<0)
/// — rapor pribadi & laporan ortu.
/// Riwayat poin satu user, terbaru dulu.
///
/// `since` Some = hanya catatan sejak waktu itu (dipakai rapor santri yang
/// sengaja dibatasi beberapa hari terakhir); None = tanpa batas waktu, hanya
/// dibatasi `limit`.
pub async fn point_history_of(
    pool: &Pool,
    user_id: i64,
    limit: i64,
    since: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<Vec<(String, i32, chrono::DateTime<chrono::Utc>)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT reason, delta, created_at FROM point_logs \
             WHERE user_id = $1 AND ($3::timestamptz IS NULL OR created_at >= $3) \
             ORDER BY created_at DESC LIMIT $2",
            &[&user_id, &limit, &since],
        )
        .await
        .context("point_history_of")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1), r.get(2))).collect())
}

/// Tambah/kurangi poin manual (dewan guru/admin) + catat di point_logs.
pub async fn adjust_points(
    pool: &Pool,
    user_id: i64,
    delta: i32,
    reason: &str,
    given_by: i64,
) -> Result<()> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("adjust_points tx")?;
    // users.points diperbarui otomatis oleh trigger trg_point_logs_balance
    // (migrasi 32) — cukup tulis point_logs.
    tx.execute(
        // category WAJIB salah satu dari CHECK migrasi 2:
        // attendance|discipline|achievement|other. 'manual' TIDAK termasuk —
        // dulu dipakai di sini, jadi SETIAP penyesuaian poin manual gagal
        // dengan constraint violation. Sifat manualnya sudah terekam di
        // `given_by` (siapa yang memberi) dan `reason` (alasannya).
        "INSERT INTO point_logs (user_id, delta, reason, category, given_by) \
         VALUES ($1, $2, $3, 'other', $4)",
        &[&user_id, &delta, &reason, &given_by],
    )
    .await
    .context("adjust_points insert")?;
    tx.commit().await.context("adjust_points commit")?;
    Ok(())
}

// ── Manajemen Kelas (admin/dewan guru/pamong) ────────────────────────────────────

pub struct ClassListRow {
    pub id: i64,
    pub name: String,
    pub description: String,
    /// kbm | bacaan | non_kbm (migrasi 65).
    pub category: Option<String>,
    /// Jenjang KBM (lambatan|cepatan|saringan|hadist_besar); None utk kategori
    /// lain — hanya KBM yang berjenjang.
    pub jenjang: Option<String>,
    pub teacher: String,
    pub member_count: i64,
    pub schedule_count: i64,
    /// Nama wali kelas; None = belum ditunjuk.
    pub wali_kelas: Option<String>,
}

/// Daftar kelas aktif + agregat (anggota unik, jumlah jadwal, pengajar terakhir).
pub async fn list_classes(pool: &Pool) -> Result<Vec<ClassListRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT c.id, c.name, COALESCE(c.description, ''), c.category, c.jenjang, \
                COALESCE((SELECT t.full_name FROM class_sessions s JOIN users t ON t.id = s.teacher_id \
                    WHERE s.class_id = c.id AND s.teacher_id IS NOT NULL \
                    ORDER BY s.session_date DESC LIMIT 1), '-'), \
                (SELECT COUNT(DISTINCT cp.user_id) FROM class_participants cp WHERE cp.class_id = c.id), \
                (SELECT COUNT(*) FROM class_schedules cs WHERE cs.class_id = c.id), \
                w.full_name \
             FROM classes c \
             LEFT JOIN users w ON w.id = c.wali_kelas_id \
             WHERE c.status = 'active' ORDER BY c.created_at DESC",
            &[],
        )
        .await
        .context("list_classes")?;
    Ok(rows
        .into_iter()
        .map(|r| ClassListRow {
            id: r.get(0),
            name: r.get(1),
            description: r.get(2),
            category: r.get(3),
            jenjang: r.get(4),
            teacher: r.get(5),
            member_count: r.get(6),
            schedule_count: r.get(7),
            wali_kelas: r.get(8),
        })
        .collect())
}

/// Kategori kelas yang sudah dipakai (DISTINCT) — untuk dropdown + ketik baru.
/// Kategori terpakai — GABUNGAN kategori kelas (Lambatan/Cepatan/dst.) DAN
/// kategori jadwal (Pengajian/Sholat/dst., migrasi 10) — satu datalist dipakai
/// utk form kelas maupun form jadwal.
pub async fn distinct_categories(pool: &Pool) -> Result<Vec<String>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT category FROM classes WHERE category IS NOT NULL AND category <> '' \
             UNION \
             SELECT category FROM class_schedules WHERE category IS NOT NULL AND category <> '' \
             ORDER BY 1",
            &[],
        )
        .await
        .context("distinct_categories")?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}

/// Jenjang kelas yang sudah dipakai (DISTINCT, migrasi 16) — untuk dropdown +
/// ketik baru (mis. "Bacaan", "Makna").
pub async fn distinct_jenjang(pool: &Pool) -> Result<Vec<String>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT jenjang FROM classes WHERE jenjang IS NOT NULL AND jenjang <> '' ORDER BY 1",
            &[],
        )
        .await
        .context("distinct_jenjang")?;
    Ok(rows.into_iter().map(|r| r.get(0)).collect())
}

/// Ubah kelas (nama + kategori + jenjang). category/jenjang kosong → NULL.
pub async fn update_class(
    pool: &Pool,
    class_id: i64,
    name: &str,
    category: Option<&str>,
    jenjang: Option<&str>,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE classes SET name = $2, category = $3, jenjang = $4, updated_at = NOW() \
             WHERE id = $1",
            &[&class_id, &name, &category, &jenjang],
        )
        .await
        .context("update_class")?;
    Ok(n > 0)
}

/// (total_kelas_aktif, total_santri_aktif).
pub async fn class_totals(pool: &Pool) -> Result<(i64, i64)> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT (SELECT COUNT(*) FROM classes WHERE status = 'active'), \
                    (SELECT COUNT(*) FROM users WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE)",
            &[],
        )
        .await
        .context("class_totals")?;
    Ok((row.get(0), row.get(1)))
}

/// Buat kelas baru (nama + kategori + jenjang, semua opsional) → id.
pub async fn create_class(
    pool: &Pool,
    name: &str,
    category: Option<&str>,
    jenjang: Option<&str>,
    wali_kelas_id: Option<i64>,
    description: &str,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            // Peran wali diuji di pernyataan yang sama — sama seperti
            // set_class_staff. Kelas KBM WAJIB punya wali (dijaga service);
            // di sini yang dijaga adalah orang yang ditunjuk memang guru aktif.
            "INSERT INTO classes (name, category, jenjang, description, wali_kelas_id) \
             SELECT $1, $2, $3, $4, $5 \
              WHERE $5::bigint IS NULL OR EXISTS ( \
                    SELECT 1 FROM users u WHERE u.id = $5 \
                      AND u.role IN ('teacher', 'dewan_guru') AND u.is_active) \
             RETURNING id",
            &[&name, &category, &jenjang, &description, &wali_kelas_id],
        )
        .await
        .context("create_class")?;
    let Some(row) = row else {
        anyhow::bail!("Guru yang dipilih sebagai wali kelas tidak valid.");
    };
    Ok(row.get(0))
}

/// Info dasar kelas + staf (wali kelas, pamong, rute izin).
pub struct ClassInfo {
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub jenjang: Option<String>,
    pub wali_kelas_id: Option<i64>,
    pub wali_kelas_name: Option<String>,
    pub require_pamong: bool,
    /// Mode verifikasi absensi kelas (migrasi 62).
    pub verify_mode: String,
    pub pamong_id: Option<i64>,
    pub pamong_name: Option<String>,
}

pub async fn class_info(pool: &Pool, class_id: i64) -> Result<Option<ClassInfo>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cl.name, COALESCE(cl.description, ''), cl.category, cl.jenjang, \
                    cl.wali_kelas_id, w.full_name, cl.require_pamong, cl.verify_mode, \
                    cl.pamong_id, pm.full_name \
             FROM classes cl \
             LEFT JOIN users w ON w.id = cl.wali_kelas_id \
             LEFT JOIN users pm ON pm.id = cl.pamong_id \
             WHERE cl.id = $1",
            &[&class_id],
        )
        .await?;
    Ok(row.map(|r| ClassInfo {
        name: r.get(0),
        description: r.get(1),
        category: r.get(2),
        jenjang: r.get(3),
        wali_kelas_id: r.get(4),
        wali_kelas_name: r.get(5),
        require_pamong: r.get(6),
        verify_mode: r.get(7),
        pamong_id: r.get(8),
        pamong_name: r.get(9),
    }))
}

/// Set wali kelas + pamong + mode verifikasi satu kelas (migrasi 29/30/62).
///
/// SATU pernyataan, dan `require_pamong` sengaja TIDAK ditulis di sini:
/// trigger `trg_sync_require_pamong` (migrasi 62) menurunkannya dari
/// `verify_mode`. Sebelumnya fungsi ini menulis `require_pamong` sendiri lalu
/// pemanggilnya menulis `verify_mode` lewat pernyataan KEDUA — dua sumber
/// kebenaran pada dua sambungan berbeda, jadi kegagalan di antara keduanya
/// meninggalkan kelas yang rute verifikasinya bertentangan dengan yang
/// tertulis di layar.
pub async fn set_class_staff(
    pool: &Pool,
    class_id: i64,
    wali_kelas_id: Option<i64>,
    pamong_id: Option<i64>,
    verify_mode: &str,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            // Peran kedua petugas diuji di sini juga — lihat set_session_teacher.
            // Syarat tambahan pada wali: kelasnya WAJIB KBM (migrasi 65).
            "UPDATE classes SET wali_kelas_id = $2, pamong_id = $3, verify_mode = $4 \
             WHERE id = $1 \
               AND ($2::bigint IS NULL OR (category = 'kbm' AND EXISTS ( \
                     SELECT 1 FROM users u WHERE u.id = $2 \
                       AND u.role IN ('teacher', 'dewan_guru') AND u.is_active))) \
               AND ($3::bigint IS NULL OR EXISTS ( \
                     SELECT 1 FROM users u WHERE u.id = $3 \
                       AND u.role = 'supervisor' AND u.is_active))",
            &[&class_id, &wali_kelas_id, &pamong_id, &verify_mode],
        )
        .await
        .context("set_class_staff")?;
    Ok(n > 0)
}

/// Opsi pamong (role supervisor) untuk dropdown wali/pamong kelas.
pub async fn pamong_options(pool: &Pool) -> Result<Vec<(i64, String)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, full_name FROM users \
             WHERE role = 'supervisor' AND is_active = TRUE ORDER BY full_name",
            &[],
        )
        .await
        .context("pamong_options")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// Santri anggota kelas (unik).
/// Anggota kelas: (id, nama, NIS, tahun angkatan).
///
/// `entry_year` ikut dibawa — dulu angkatan ditebak dari empat digit awal NIS
/// (`service::kelas::angkatan_from_nis`), dan itu berhenti bekerja begitu NIS
/// resmi pondok dipakai: `500032760078240001` diawali "5000", bukan tahun, jadi
/// angkatannya kosong untuk SEMUA santri. Kolomnya sendiri sudah terisi sejak
/// impor daftar induk (migrasi 74) — tinggal dibaca.
pub async fn class_members(
    pool: &Pool,
    class_id: i64,
) -> Result<Vec<(i64, String, Option<String>, Option<i16>)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT u.id, u.full_name, u.nis, u.entry_year \
             FROM class_participants cp JOIN users u ON u.id = cp.user_id \
             WHERE cp.class_id = $1 AND u.role IN ('santri', 'santri_finance') ORDER BY u.full_name",
            &[&class_id],
        )
        .await
        .context("class_members")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
        .collect())
}

pub struct SchedRow {
    pub id: i64,
    pub title: String,
    pub start_time: chrono::NaiveTime,
    pub end_time: chrono::NaiveTime,
    pub limit_time: chrono::NaiveTime,
    pub recurrence_type: String,
    pub start_date: chrono::NaiveDate,
    pub end_date: Option<chrono::NaiveDate>,
    /// Kategori jadwal (mis. "Pengajian"/"Sholat") — override kategori kelas
    /// utk sesi yang lahir dari jadwal ini (lihat migrasi 10, gerbang rekam
    /// suara di models::category_allows_recording).
    pub category: Option<String>,
    /// Poin BONUS saat TEPAT WAKTU (migrasi 21). None = default 10. Ditambahkan.
    pub present_points: Option<i16>,
    /// Poin DIPOTONG saat TERLAMBAT (migrasi 13/21). None = default 0. Magnitude
    /// positif, dikurangkan.
    pub late_points: Option<i16>,
    /// Poin DIPOTONG saat ALPA (migrasi 15). None = default 15. Magnitude
    /// positif, dikurangkan.
    pub absent_points: Option<i16>,
    /// Jenis kegiatan PRD (migrasi 28): kbm|non_kbm|piket|apel_kepulangan. None =
    /// legacy (preset default 10/0/15). Menentukan preset poin bila override kosong.
    pub activity_type: Option<String>,
    /// Poin DIPOTONG saat IZIN biasa (migrasi 28). None = preset kategori.
    pub izin_points: Option<i16>,
    /// Ruang = perangkat RFID (migrasi 24). None = belum diset. room_name utk
    /// tampilan (join rfid_devices.device_name).
    pub room_id: Option<i64>,
    pub room_name: Option<String>,
    /// Tanggal manual (migrasi 23) untuk recurrence 'custom' — ISO "YYYY-MM-DD".
    /// Kosong utk pola biasa (harian/mingguan/bulanan/sekali).
    pub custom_dates: Vec<String>,
    /// Materi yang SEDANG BERJALAN (migrasi 57) — pointer yang maju tiap
    /// pertemuan. None = belum diset.
    pub current_book_id: Option<i64>,
    pub current_book_title: Option<String>,
    pub current_book_category: Option<String>,
    pub current_book_surahs: Option<serde_json::Value>,
    /// Posisi milik JADWAL INI sendiri — "jadwal ini sudah sampai mana".
    /// Berbeda dari posisi di `curriculum`, yang mewakili kemajuan KELAS secara
    /// keseluruhan atas materi itu. Dua jadwal boleh membaca kitab sama di
    /// titik berbeda (mis. kelas pagi ayat 50, kelas malam ayat 30).
    pub current_surah: Option<i16>,
    pub current_unit: Option<i32>,
}

/// Opsi ruang (perangkat RFID) untuk dropdown jadwal — hanya id + nama (tanpa
/// api_key, aman dikirim ke semua peran staf).
/// Perangkat yang boleh dipilih sebagai RUANG jadwal kelas.
///
/// GERBANG UTAMA sengaja DIKECUALIKAN: tap di sana selalu diartikan
/// keluar/masuk area pondok, tak pernah jadi absensi kelas. Kalau ia boleh
/// dipilih sebagai ruang, kelas itu jadi mustahil diabsen — tap di gerbang
/// hanya men-toggle, tap di tempat lain ditolak karena bukan ruangnya. Diam-
/// diam, tanpa pesan error.
pub async fn device_options(pool: &Pool) -> Result<Vec<(i64, String)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, device_name FROM rfid_devices \
             WHERE category <> 'gate_utama' ORDER BY device_name",
            &[],
        )
        .await
        .context("device_options")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

fn json_dates(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

/// Jadwal-jadwal milik kelas.
pub async fn class_schedules(pool: &Pool, class_id: i64) -> Result<Vec<SchedRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT cs.id, COALESCE(cs.title, ''), cs.start_time, cs.end_time, \
                    cs.limit_entery_time, cs.recurrence_type, cs.start_date, cs.end_date, \
                    cs.category, cs.present_points, cs.late_points, cs.absent_points, \
                    cs.room_id, dev.device_name, cs.custom_dates, cs.activity_type, cs.izin_points, \
                    cs.current_book_id, cb.title, cb.category, cb.surahs, \
                    cs.current_surah, cs.current_unit \
             FROM class_schedules cs \
             LEFT JOIN rfid_devices dev ON dev.id = cs.room_id \
             LEFT JOIN books cb ON cb.id = cs.current_book_id \
             WHERE cs.class_id = $1 ORDER BY cs.start_time",
            &[&class_id],
        )
        .await
        .context("class_schedules")?;
    Ok(rows
        .into_iter()
        .map(|r| SchedRow {
            id: r.get(0),
            title: r.get(1),
            start_time: r.get(2),
            end_time: r.get(3),
            limit_time: r.get(4),
            recurrence_type: r.get(5),
            start_date: r.get(6),
            end_date: r.get(7),
            category: r.get(8),
            present_points: r.get(9),
            late_points: r.get(10),
            absent_points: r.get(11),
            room_id: r.get(12),
            room_name: r.get(13),
            custom_dates: json_dates(&r.get::<_, serde_json::Value>(14)),
            activity_type: r.get(15),
            izin_points: r.get(16),
            current_book_id: r.get(17),
            current_book_title: r.get(18),
            current_book_category: r.get(19),
            current_book_surahs: r.get(20),
            current_surah: r.get(21),
            current_unit: r.get(22),
        })
        .collect())
}

/// Setel materi & posisi yang SEDANG BERJALAN pada satu jadwal (migrasi 57).
/// `book_id` None = lepaskan penanda (surat/unit ikut dikosongkan).
pub async fn set_schedule_current(
    pool: &Pool,
    schedule_id: i64,
    book_id: Option<i64>,
    surah: Option<i16>,
    unit: Option<i32>,
) -> Result<bool> {
    let c = pool.get().await?;
    // Materi dilepas → posisinya ikut dikosongkan; kalau tidak, angka lama
    // menggantung tanpa materi dan terbaca seolah masih berlaku.
    let (surah, unit) = if book_id.is_none() { (None, None) } else { (surah, unit) };
    let n = c
        .execute(
            "UPDATE class_schedules \
                SET current_book_id = $2, current_surah = $3, current_unit = $4 \
              WHERE id = $1",
            &[&schedule_id, &book_id, &surah, &unit],
        )
        .await
        .context("set_schedule_current")?;
    Ok(n > 0)
}

/// Apakah materi `book_id` ada di kurikulum KELAS pemilik jadwal ini?
///
/// Penjagaan sisi server untuk aturan "materi jadwal hanya dari kurikulum
/// kelas". Dropdown di UI sudah disaring, tapi request bisa dikirim langsung.
pub async fn schedule_book_in_curriculum(
    pool: &Pool,
    schedule_id: i64,
    book_id: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT 1 FROM class_schedules cs \
               JOIN curriculum cu ON cu.class_id = cs.class_id AND cu.book_id = $2 \
              WHERE cs.id = $1 LIMIT 1",
            &[&schedule_id, &book_id],
        )
        .await
        .context("schedule_book_in_curriculum")?;
    Ok(row.is_some())
}

/// Buat jadwal baru → id.
#[allow(clippy::too_many_arguments)]
pub async fn create_schedule(
    pool: &Pool,
    class_id: i64,
    title: &str,
    start_time: chrono::NaiveTime,
    end_time: chrono::NaiveTime,
    limit_time: chrono::NaiveTime,
    recurrence_type: &str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
    category: Option<&str>,
    present_points: Option<i16>,
    late_points: Option<i16>,
    absent_points: Option<i16>,
    room_id: Option<i64>,
    custom_dates: &serde_json::Value,
    activity_type: Option<&str>,
    izin_points: Option<i16>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO class_schedules \
                (class_id, title, start_time, end_time, limit_entery_time, recurrence_type, \
                 start_date, end_date, category, present_points, late_points, absent_points, \
                 room_id, custom_dates, activity_type, izin_points) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16) RETURNING id",
            &[
                &class_id, &title, &start_time, &end_time, &limit_time, &recurrence_type,
                &start_date, &end_date, &category, &present_points, &late_points, &absent_points,
                &room_id, custom_dates, &activity_type, &izin_points,
            ],
        )
        .await
        .context("create_schedule")?;
    Ok(row.get(0))
}

/// Buat sesi baru → id. `book_id` opsional (sesi non-mengaji spt Sholat tak
/// perlu materi buku); `book_pages` JSONB array pasangan halaman (migrasi 20).
#[allow(clippy::too_many_arguments)]
pub async fn create_session(
    pool: &Pool,
    class_id: i64,
    schedule_id: Option<i64>,
    teacher_id: Option<i64>,
    title: &str,
    session_date: chrono::NaiveDate,
    book_id: Option<i64>,
    book_pages: &serde_json::Value,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            // WHERE NOT EXISTS(jadwal milik kelas LAIN): sesi tak boleh
            // menggendong jadwal kelas orang. Kombinasi itu merusak hampir
            // semua yang membacanya — jam mulai, batas terlambat, poin, ruang,
            // dan siapa peserta yang di-auto-alpa — dan tak ada satu pun yang
            // menyadarinya karena kedua id-nya sah masing-masing.
            "INSERT INTO class_sessions \
                (class_id, class_schedule_id, teacher_id, title, session_date, book_id, book_pages) \
             SELECT $1, $2, $3, $4, $5, $6, $7 \
              WHERE $2::bigint IS NULL OR EXISTS ( \
                    SELECT 1 FROM class_schedules sch \
                     WHERE sch.id = $2 AND sch.class_id = $1) \
             RETURNING id",
            &[
                &class_id, &schedule_id, &teacher_id, &title, &session_date, &book_id, book_pages,
            ],
        )
        .await
        .context("create_session")?;
    let Some(row) = row else {
        anyhow::bail!("Jadwal yang dipilih bukan milik kelas ini.");
    };
    Ok(row.get(0))
}

/// Tambah santri ke KELAS. Satu baris per (kelas, santri) sejak migrasi 61 —
/// keanggotaan berlaku untuk SEMUA jadwal kelas itu, termasuk jadwal yang baru
/// dibuat kemudian. Return true bila baru (bukan duplikat).
pub async fn add_member(
    pool: &Pool,
    class_id: i64,
    user_id: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            // Hanya SANTRI yang boleh jadi peserta. Tanpa syarat ini seorang
            // guru bisa didaftarkan sebagai peserta kelasnya sendiri lalu ikut
            // terseret auto-alpa, papan poin, dan tagihan.
            "INSERT INTO class_participants (class_id, user_id) \
             SELECT $1, u.id FROM users u \
              WHERE u.id = $2 AND u.role IN ('santri', 'santri_finance') AND u.is_active \
             ON CONFLICT (class_id, user_id) DO NOTHING",
            &[&class_id, &user_id],
        )
        .await
        .context("add_member")?;
    Ok(n > 0)
}

/// Keluarkan santri dari kelas KBM LAIN — prasyarat memindahkan mereka ke
/// kelas KBM baru.
///
/// Satu santri hanya boleh satu kelas KBM, dijaga trigger `trg_satu_kelas_kbm`
/// (migrasi 65). Tanpa langkah ini, menambahkan santri yang sudah punya kelas
/// KBM akan DITOLAK database dengan `unique_violation` — dan pengelola cuma
/// melihat penambahan yang gagal tanpa tahu sebabnya.
///
/// Hanya menyentuh keanggotaan KBM: kelas non-KBM (piket, apel, sholat) tak
/// terbatas jumlahnya dan tak boleh ikut terhapus. Riwayat kehadiran juga aman
/// — `attendances` menunjuk ke sesi, bukan ke baris keanggotaan.
///
/// Tak melakukan apa pun bila `class_id` bukan kelas KBM.
pub async fn keluarkan_dari_kbm_lain(
    tx: &deadpool_postgres::Transaction<'_>,
    class_id: i64,
    user_ids: &[i64],
) -> Result<u64> {
    let n = tx
        .execute(
            "DELETE FROM class_participants cp \
              USING classes c \
              WHERE c.id = cp.class_id \
                AND c.category = 'kbm' \
                AND cp.user_id = ANY($2::bigint[]) \
                AND cp.class_id <> $1 \
                AND EXISTS (SELECT 1 FROM classes t \
                             WHERE t.id = $1 AND t.category = 'kbm')",
            &[&class_id, &user_ids],
        )
        .await
        .context("keluarkan_dari_kbm_lain")?;
    Ok(n)
}

/// Tambah BANYAK santri ke KELAS sekali jalan. Set-based via unnest;
/// ON CONFLICT skip yang sudah terdaftar. Return jumlah BARU.
pub async fn add_members(
    pool: &Pool,
    class_id: i64,
    user_ids: &[i64],
    pindahkan: bool,
) -> Result<i64> {
    if user_ids.is_empty() {
        return Ok(0);
    }
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("add_members tx")?;
    // Pemindahan dan penambahan HARUS satu transaksi: kalau keanggotaan KBM
    // lama terhapus lalu penambahannya gagal, santri berakhir tanpa kelas KBM
    // sama sekali — lebih buruk daripada penambahan yang sekadar ditolak.
    if pindahkan {
        keluarkan_dari_kbm_lain(&tx, class_id, user_ids).await?;
    }
    let n = tx
        .execute(
            // Disaring lewat users: id yang bukan santri aktif dijatuhkan
            // (alasan sama dengan add_member).
            "INSERT INTO class_participants (class_id, user_id) \
             SELECT $1, u.id FROM users u \
              WHERE u.id = ANY($2::bigint[]) \
                AND u.role IN ('santri', 'santri_finance') AND u.is_active \
             ON CONFLICT (class_id, user_id) DO NOTHING",
            &[&class_id, &user_ids],
        )
        .await
        .context("add_members")?;
    tx.commit().await.context("add_members commit")?;
    Ok(n as i64)
}

/// Opsi pengajar (teacher/dewan_guru/supervisor) untuk dropdown buat sesi.
pub async fn teacher_options(pool: &Pool) -> Result<Vec<(i64, String)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, full_name FROM users \
             WHERE role IN ('teacher','dewan_guru','supervisor') AND is_active = TRUE \
             ORDER BY full_name",
            &[],
        )
        .await
        .context("teacher_options")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// Ubah jadwal (title/jam/recurrence/tanggal). Return true bila ada baris ter-update.
#[allow(clippy::too_many_arguments)]
pub async fn update_schedule(
    pool: &Pool,
    schedule_id: i64,
    title: &str,
    start_time: chrono::NaiveTime,
    end_time: chrono::NaiveTime,
    limit_time: chrono::NaiveTime,
    recurrence_type: &str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
    category: Option<&str>,
    present_points: Option<i16>,
    late_points: Option<i16>,
    absent_points: Option<i16>,
    room_id: Option<i64>,
    custom_dates: &serde_json::Value,
    activity_type: Option<&str>,
    izin_points: Option<i16>,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_schedules SET title = $2, start_time = $3, end_time = $4, \
                limit_entery_time = $5, recurrence_type = $6, start_date = $7, end_date = $8, \
                category = $9, present_points = $10, late_points = $11, absent_points = $12, \
                room_id = $13, custom_dates = $14, activity_type = $15, izin_points = $16 \
             WHERE id = $1",
            &[
                &schedule_id,
                &title,
                &start_time,
                &end_time,
                &limit_time,
                &recurrence_type,
                &start_date,
                &end_date,
                &category,
                &present_points,
                &late_points,
                &absent_points,
                &room_id,
                custom_dates,
                &activity_type,
                &izin_points,
            ],
        )
        .await
        .context("update_schedule")?;
    Ok(n > 0)
}

/// Hapus jadwal. (class_sessions.class_schedule_id → SET? kolom nullable, ON DELETE
/// default NO ACTION → hapus manual referensi dulu agar aman.)
/// Hapus jadwal + sesi MENDATANG-nya (≥ `today`) yang belum dipakai (tak ada
/// absensi/chat) → tak meninggalkan sesi "yatim" yang membingungkan. Sesi lampau
/// atau yang sudah ada absensi/chat DILEPAS (class_schedule_id=NULL) agar histori
/// absensi tetap utuh. Semua dalam satu transaksi.
pub async fn delete_schedule(
    pool: &Pool,
    schedule_id: i64,
    today: chrono::NaiveDate,
) -> Result<bool> {
    let mut c = pool.get().await?;
    let tx = c.transaction().await.context("delete_schedule tx")?;

    // Hapus sesi mendatang yang AMAN (belum ada absensi & chat).
    tx.execute(
        "DELETE FROM class_sessions cs \
         WHERE cs.class_schedule_id = $1 AND cs.session_date >= $2 \
           AND NOT EXISTS (SELECT 1 FROM attendances a WHERE a.class_session_id = cs.id) \
           AND NOT EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id = cs.id)",
        &[&schedule_id, &today],
    )
    .await
    .context("delete_schedule hapus sesi mendatang")?;

    // Sisanya (lampau / sudah dipakai) → dilepas, histori tetap ada.
    tx.execute(
        "UPDATE class_sessions SET class_schedule_id = NULL WHERE class_schedule_id = $1",
        &[&schedule_id],
    )
    .await
    .context("delete_schedule lepas sesi")?;

    let n = tx
        .execute("DELETE FROM class_schedules WHERE id = $1", &[&schedule_id])
        .await
        .context("delete_schedule")?;
    tx.commit().await.context("delete_schedule commit")?;
    Ok(n > 0)
}

/// Hapus sesi MENDATANG (≥ `today`) milik jadwal ini yang tanggalnya TIDAK ada di
/// `valid` (tanggal-tanggal yang sah menurut jadwal terbaru) DAN belum dipakai
/// (tanpa absensi/chat). Dipakai setelah update jadwal agar sesi mendatang
/// mengikuti rentang/pola baru; sesi dalam rentang (mis. sudah diberi pengajar /
/// ditandai libur) dibiarkan. Return jumlah sesi terhapus.
pub async fn delete_future_sessions_not_in(
    pool: &Pool,
    schedule_id: i64,
    today: chrono::NaiveDate,
    valid: &[chrono::NaiveDate],
) -> Result<u64> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "DELETE FROM class_sessions cs \
             WHERE cs.class_schedule_id = $1 AND cs.session_date >= $2 \
               AND NOT (cs.session_date = ANY($3::date[])) \
               AND NOT EXISTS (SELECT 1 FROM attendances a WHERE a.class_session_id = cs.id) \
               AND NOT EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id = cs.id)",
            &[&schedule_id, &today, &valid],
        )
        .await
        .context("delete_future_sessions_not_in")?;
    Ok(n)
}

/// Info sebuah jadwal (untuk generate sesi): (class_id, start_time, recurrence, start_date).
pub async fn schedule_info(
    pool: &Pool,
    schedule_id: i64,
) -> Result<Option<(i64, String, String, chrono::NaiveDate)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT class_id, COALESCE(title, ''), recurrence_type, start_date \
             FROM class_schedules WHERE id = $1",
            &[&schedule_id],
        )
        .await?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))))
}

/// Insert BANYAK sesi sekaligus (generate bulanan/mendatang) dalam SATU query
/// set-based (`unnest` + `NOT EXISTS`) — cepat & idempotent, tak menggandakan
/// (schedule, tanggal) yang sudah ada. Return jumlah sesi baru.
pub async fn insert_sessions(
    pool: &Pool,
    class_id: i64,
    schedule_id: i64,
    title: &str,
    dates: &[chrono::NaiveDate],
) -> Result<i64> {
    if dates.is_empty() {
        return Ok(0);
    }
    let c = pool.get().await?;
    let n = c
        .execute(
            // ON CONFLICT, BUKAN "WHERE NOT EXISTS": yang terakhir tak atomik —
            // job latar dan update_schedule yang berjalan bersamaan sama-sama
            // membaca "belum ada" lalu sama-sama menyisipkan. Constraint
            // uq_session_schedule_date (migrasi 52) yang jadi wasitnya.
            "INSERT INTO class_sessions (class_id, class_schedule_id, title, session_date) \
             SELECT $1, $2, $3, d FROM unnest($4::date[]) AS d \
             ON CONFLICT (class_schedule_id, session_date) \
                WHERE class_schedule_id IS NOT NULL DO NOTHING",
            &[&class_id, &schedule_id, &title, &dates],
        )
        .await
        .context("insert_sessions")?;
    Ok(n as i64)
}

/// Apakah kelas ini KBM? Penentu apakah ia boleh punya wali kelas, berjenjang,
/// dan boleh direkam (migrasi 65). Kelas tak ada → false.
pub async fn kelas_adalah_kbm(pool: &Pool, class_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM classes WHERE id = $1 AND category = 'kbm')",
            &[&class_id],
        )
        .await
        .context("kelas_adalah_kbm")?;
    Ok(row.get(0))
}

/// Apakah `user_id` PETUGAS kelas ini — wali kelasnya atau pamongnya?
///
/// Batas wewenang untuk hal-hal yang menyangkut isi kelas: mengisi kurikulum,
/// menandai materi yang sedang berjalan, dan menunjuk guru/pamong tiap sesi.
/// Bukan "peran guru" secara umum: guru kelas lain tak berkepentingan di sini,
/// dan sebelum ini siapa pun ber-peran guru/pamong bisa menyunting kurikulum
/// kelas mana pun.
pub async fn petugas_kelas(pool: &Pool, class_id: i64, user_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM classes \
                             WHERE id = $1 AND (wali_kelas_id = $2 OR pamong_id = $2))",
            &[&class_id, &user_id],
        )
        .await
        .context("petugas_kelas")?;
    Ok(row.get(0))
}

/// Kelas pemilik sebuah baris kurikulum — untuk menguji wewenang dari id
/// kurikulum saja (sunting/hapus tak membawa class_id).
pub async fn kelas_dari_kurikulum(pool: &Pool, curriculum_id: i64) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT class_id FROM curriculum WHERE id = $1", &[&curriculum_id])
        .await
        .context("kelas_dari_kurikulum")?;
    Ok(row.map(|r| r.get(0)))
}

/// Apakah `user_id` PAMONG kelas ini? Lebih sempit dari [`petugas_kelas`] —
/// wali kelas sengaja tidak termasuk. Dipakai untuk menata jadwal & anggota.
pub async fn pamong_kelas(pool: &Pool, class_id: i64, user_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM classes WHERE id = $1 AND pamong_id = $2)",
            &[&class_id, &user_id],
        )
        .await
        .context("pamong_kelas")?;
    Ok(row.get(0))
}

/// Kelas pemilik sebuah jadwal — untuk menguji wewenang dari id jadwal saja.
pub async fn kelas_dari_jadwal(pool: &Pool, schedule_id: i64) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT class_id FROM class_schedules WHERE id = $1", &[&schedule_id])
        .await
        .context("kelas_dari_jadwal")?;
    Ok(row.map(|r| r.get(0)))
}

/// Kelas pemilik sebuah sesi — untuk menguji wewenang dari id sesi saja.
pub async fn kelas_dari_sesi(pool: &Pool, session_id: i64) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT class_id FROM class_sessions WHERE id = $1", &[&session_id])
        .await
        .context("kelas_dari_sesi")?;
    Ok(row.map(|r| r.get(0)))
}

/// Apakah kelas ini sudah punya wali? Dipakai menolak perpindahan kategori ke
/// KBM sebelum walinya ditetapkan (wali KBM wajib).
pub async fn kelas_punya_wali(pool: &Pool, class_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS (SELECT 1 FROM classes WHERE id = $1 AND wali_kelas_id IS NOT NULL)",
            &[&class_id],
        )
        .await
        .context("kelas_punya_wali")?;
    Ok(row.get(0))
}

/// Apakah sesi ini milik kelas KBM? Gerbang siaran & rekaman suara.
///
/// Sengaja membaca kategori KELAS, bukan `COALESCE(jadwal.category, kelas)`
/// seperti `session_category`: kolom kategori jadwal itu teks bebas penimpa
/// JUDUL sesi, dan mengizinkan rekaman berdasarkan kata yang diketik orang di
/// sana berarti kelas piket bisa ikut merekam hanya karena judulnya menyebut
/// "pengajian".
pub async fn sesi_kelas_kbm(pool: &Pool, session_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS ( \
                SELECT 1 FROM class_sessions s JOIN classes cl ON cl.id = s.class_id \
                 WHERE s.id = $1 AND cl.category = 'kbm')",
            &[&session_id],
        )
        .await
        .context("sesi_kelas_kbm")?;
    Ok(row.get(0))
}

/// Kategori (`kbm`/`bacaan`/`non_kbm`) beberapa kelas sekaligus, untuk pelabelan.
pub async fn kategori_kelas(
    pool: &Pool,
    class_ids: &[i64],
) -> Result<std::collections::HashMap<i64, String>> {
    if class_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, category FROM classes WHERE id = ANY($1::bigint[])",
            &[&class_ids],
        )
        .await
        .context("kategori_kelas")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// Dari daftar id, siapa saja yang SUDAH punya kelas KBM selain `class_id`?
///
/// Return `(nama santri, nama kelas KBM-nya)`. Kosong = semuanya boleh masuk.
/// Hanya berarti bila `class_id` sendiri kelas KBM — untuk kelas non-KBM
/// (piket, apel, sholat) tak ada batas jumlah dan query ini tak mengembalikan
/// apa pun.
pub async fn santri_dengan_kbm_lain(
    pool: &Pool,
    class_id: i64,
    user_ids: &[i64],
) -> Result<Vec<(String, String)>> {
    if user_ids.is_empty() {
        return Ok(Vec::new());
    }
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, c2.name \
             FROM users u \
             JOIN class_participants cp ON cp.user_id = u.id \
             JOIN classes c2 ON c2.id = cp.class_id AND c2.category = 'kbm' \
             WHERE u.id = ANY($2::bigint[]) AND cp.class_id <> $1 \
               AND EXISTS (SELECT 1 FROM classes tgt \
                            WHERE tgt.id = $1 AND tgt.category = 'kbm') \
             ORDER BY u.full_name",
            &[&class_id, &user_ids],
        )
        .await
        .context("santri_dengan_kbm_lain")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
}

/// Keluarkan santri dari kelas (semua barisnya lintas-jadwal).
pub async fn remove_member(pool: &Pool, class_id: i64, user_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "DELETE FROM class_participants WHERE class_id = $1 AND user_id = $2",
            &[&class_id, &user_id],
        )
        .await
        .context("remove_member")?;
    Ok(n > 0)
}

/// Set/ubah pengajar sebuah sesi (teacher_id NULL bila 0/None).
pub async fn set_session_teacher(
    pool: &Pool,
    session_id: i64,
    teacher_id: Option<i64>,
) -> Result<bool> {
    let c = pool.get().await?;
    // Peran targetnya diuji DI DALAM pernyataan yang sama. Dropdown UI memang
    // hanya menawarkan guru, tapi request-nya bisa dirakit sendiri dengan id
    // siapa pun — dan "guru sesi" menentukan siapa yang boleh menyiarkan,
    // mengoreksi absensi, dan memverifikasi. Memeriksanya lebih dulu lalu
    // meng-UPDATE tak sama amannya: di antara keduanya peran bisa berubah.
    let n = c
        .execute(
            "UPDATE class_sessions SET teacher_id = $2 WHERE id = $1 \
               AND ($2::bigint IS NULL OR EXISTS ( \
                     SELECT 1 FROM users u WHERE u.id = $2 \
                       AND u.role IN ('teacher', 'dewan_guru') AND u.is_active))",
            &[&session_id, &teacher_id],
        )
        .await
        .context("set_session_teacher")?;
    Ok(n > 0)
}

/// Set pamong bertugas verifikasi untuk satu sesi (migrasi 33). None = kosongkan
/// (fallback ke pamong kelas).
pub async fn set_session_pamong(
    pool: &Pool,
    session_id: i64,
    pamong_id: Option<i64>,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            // Alasan sama dengan set_session_teacher: pamong sesi menentukan
            // siapa yang berhak memverifikasi absensinya.
            "UPDATE class_sessions SET pamong_id = $2 WHERE id = $1 \
               AND ($2::bigint IS NULL OR EXISTS ( \
                     SELECT 1 FROM users u WHERE u.id = $2 \
                       AND u.role = 'supervisor' AND u.is_active))",
            &[&session_id, &pamong_id],
        )
        .await
        .context("set_session_pamong")?;
    Ok(n > 0)
}

/// Set/ubah materi buku sesi (book_id NULL bila 0/None; migrasi 20).
pub async fn set_session_book(
    pool: &Pool,
    session_id: i64,
    book_id: Option<i64>,
    book_pages: &serde_json::Value,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_sessions SET book_id = $2, book_pages = $3 WHERE id = $1",
            &[&session_id, &book_id, book_pages],
        )
        .await
        .context("set_session_book")?;
    Ok(n > 0)
}

/// Set materi TARGET/rencana sesi (migrasi 41). None = kosongkan.
pub async fn set_session_target(
    pool: &Pool,
    session_id: i64,
    target_book_id: Option<i64>,
    target_pages: &serde_json::Value,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_sessions SET target_book_id = $2, target_pages = $3 WHERE id = $1",
            &[&session_id, &target_book_id, target_pages],
        )
        .await
        .context("set_session_target")?;
    Ok(n > 0)
}

/// Set catatan ayat/hadith AKTUAL sesi (teks bebas, migrasi 41).
pub async fn set_session_actual_detail(
    pool: &Pool,
    session_id: i64,
    detail: &str,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_sessions SET actual_detail = $2 WHERE id = $1",
            &[&session_id, &detail],
        )
        .await
        .context("set_session_actual_detail")?;
    Ok(n > 0)
}

/// Set status sesi (mis. 'cancelled' = libur, 'scheduled' = aktif kembali).
pub async fn set_session_status(pool: &Pool, session_id: i64, status: &str) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE class_sessions SET status = $2 WHERE id = $1",
            &[&session_id, &status],
        )
        .await
        .context("set_session_status")?;
    Ok(n > 0)
}

/// Beberapa santri aktif (untuk daftar default form Tambah Santri, tanpa cari).
pub async fn some_students(pool: &Pool, limit: i64) -> Result<Vec<super::parents::StudentRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, cl.name \
             FROM users u \
             LEFT JOIN classes cl ON cl.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
             ORDER BY u.full_name LIMIT $1",
            &[&limit],
        )
        .await
        .context("some_students")?;
    Ok(rows
        .into_iter()
        .map(|r| super::parents::StudentRow {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
        })
        .collect())
}

/// Cari santri untuk DITAMBAHKAN ke `class_id` — mengecualikan yang SUDAH jadi
/// anggota kelas itu. `q` kosong/pendek → daftar default. Dipakai AddMemberForm.
pub async fn students_not_in_class(
    pool: &Pool,
    class_id: i64,
    q: &str,
    angkatan: Option<i16>,
    limit: i64,
) -> Result<Vec<CalonSantri>> {
    let c = pool.get().await?;
    let qt = q.trim();
    let pattern = format!("%{}%", qt);
    let rows = c
        .query(
            // `kbm.name` DIPISAH dari `cl.name`: yang pertama menentukan boleh
            // tidaknya santri masuk kelas KBM lain (trigger migrasi 65), yang
            // kedua sekadar kelas mana saja sebagai keterangan. Menyatukan
            // keduanya membuat santri yang cuma ikut piket tampak seperti sudah
            // punya kelas KBM.
            "SELECT u.id, u.full_name, u.nis, cl.name, kbm.name, u.entry_year \
             FROM users u \
             LEFT JOIN classes cl ON cl.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 WHERE cp.user_id = u.id ORDER BY cp.class_id LIMIT 1 \
             ) \
             LEFT JOIN classes kbm ON kbm.id = ( \
                 SELECT cp.class_id FROM class_participants cp \
                 JOIN classes c2 ON c2.id = cp.class_id \
                 WHERE cp.user_id = u.id AND c2.category = 'kbm' LIMIT 1 \
             ) \
             WHERE u.role IN ('santri', 'santri_finance') AND u.is_active = TRUE \
               AND u.id NOT IN ( \
                   SELECT cp2.user_id FROM class_participants cp2 WHERE cp2.class_id = $1 \
               ) \
               AND ($2 = '' OR u.full_name ILIKE $3 OR u.nis = $2) \
               AND ($5::int2 IS NULL OR u.entry_year = $5) \
             ORDER BY u.full_name LIMIT $4",
            &[&class_id, &qt, &pattern, &limit, &angkatan],
        )
        .await
        .context("students_not_in_class")?;
    Ok(rows
        .into_iter()
        .map(|r| CalonSantri {
            id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kbm_class: r.get(4),
            entry_year: r.get(5),
        })
        .collect())
}

/// Satu calon anggota kelas di pemilih "Tambah Santri".
pub struct CalonSantri {
    pub id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    /// Kelas mana pun yang ia ikuti — sekadar keterangan.
    pub class_name: Option<String>,
    /// Kelas KBM-nya bila sudah punya; penentu apakah ia harus DIPINDAH.
    pub kbm_class: Option<String>,
    pub entry_year: Option<i16>,
}

/// Kategori sebuah kelas (`kbm` | `non_kbm` | `bacaan`), atau None bila kelasnya
/// tak ada. Dipakai menurunkan jenis kegiatan jadwal — lihat
/// `service::kelas::jenis_dari_kategori_kelas`.
pub async fn class_category(pool: &Pool, class_id: i64) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt("SELECT category FROM classes WHERE id = $1", &[&class_id])
        .await
        .context("class_category")?;
    Ok(row.and_then(|r| r.get::<_, Option<String>>(0)))
}

/// Kategori kelas PEMILIK sebuah jadwal. `update_schedule` hanya memegang
/// `schedule_id`, jadi kelasnya dicari lewat jadwalnya.
pub async fn class_category_by_schedule(
    pool: &Pool,
    schedule_id: i64,
) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT c.category FROM class_schedules s \
              JOIN classes c ON c.id = s.class_id WHERE s.id = $1",
            &[&schedule_id],
        )
        .await
        .context("class_category_by_schedule")?;
    Ok(row.and_then(|r| r.get::<_, Option<String>>(0)))
}

/// Jadwal aktif sebuah kelas untuk auto-generate sesi mendatang.
// Tuple jadwal aktif untuk materialisasi: (..., start_date, end_date). end_date
// WAJIB dibawa agar materialisasi TIDAK melewati akhir jadwal.
type ActiveSched = (
    i64,
    String,
    String,
    chrono::NaiveDate,
    Option<chrono::NaiveDate>,
);

pub async fn active_schedules_of(pool: &Pool, class_id: i64) -> Result<Vec<ActiveSched>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT id, COALESCE(title, ''), recurrence_type, start_date, end_date \
             FROM class_schedules WHERE class_id = $1 AND status = 'active'",
            &[&class_id],
        )
        .await
        .context("active_schedules_of")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
        .collect())
}

/// Semua jadwal aktif LINTAS-kelas (class_id, id, title, recurrence, start_date,
/// end_date) — untuk materialisasi sesi di task background (bukan per-request).
pub async fn active_schedules_all(
    pool: &Pool,
) -> Result<
    Vec<(
        i64,
        i64,
        String,
        String,
        chrono::NaiveDate,
        Option<chrono::NaiveDate>,
    )>,
> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT class_id, id, COALESCE(title, ''), recurrence_type, start_date, end_date \
             FROM class_schedules WHERE status = 'active'",
            &[],
        )
        .await
        .context("active_schedules_all")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4), r.get(5)))
        .collect())
}

// ── Kurikulum (migrasi 17) ───────────────────────────────────────────────────

pub struct CurriculumRow {
    pub id: i64,
    pub title: String,
    pub order_index: i16,
    /// Tautan ke materi terdaftar (migrasi 22) — None = materi bebas-teks
    /// (hanya baris lama; kurikulum baru wajib tertaut).
    pub book_id: Option<i64>,
    pub book_title: Option<String>,
    pub book_category: Option<String>,
    /// `books.surahs` mentah — dipakai service menyusun label rentang.
    pub book_surahs: Option<serde_json::Value>,
    /// `books.total_pages` — pembagi saat rentang dikosongkan (seluruh materi).
    pub book_total_pages: Option<i32>,
    /// Rentang terstruktur (migrasi 57). None = belum diisi.
    pub start_surah: Option<i16>,
    pub start_unit: Option<i32>,
    pub end_surah: Option<i16>,
    pub end_unit: Option<i32>,
    /// Sudah sampai mana (migrasi 59) — dasar progres & status.
    pub current_surah: Option<i16>,
    pub current_unit: Option<i32>,
}

/// Cakupan materi/kitab kelas ini, terurut sesuai order_index.
pub async fn class_curriculum(pool: &Pool, class_id: i64) -> Result<Vec<CurriculumRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT cu.id, cu.title, \
                    cu.order_index, cu.book_id, b.title, \
                    b.category, b.surahs, b.total_pages, \
                    cu.start_surah, cu.start_unit, cu.end_surah, cu.end_unit, \
                    cu.current_surah, cu.current_unit \
             FROM curriculum cu \
             LEFT JOIN books b ON b.id = cu.book_id \
             WHERE cu.class_id = $1 ORDER BY cu.order_index, cu.id",
            &[&class_id],
        )
        .await
        .context("class_curriculum")?;
    Ok(rows
        .into_iter()
        .map(|r| CurriculumRow {
            id: r.get(0),
            title: r.get(1),
            order_index: r.get(2),
            book_id: r.get(3),
            book_title: r.get(4),
            book_category: r.get(5),
            book_surahs: r.get(6),
            book_total_pages: r.get(7),
            start_surah: r.get(8),
            start_unit: r.get(9),
            end_surah: r.get(10),
            end_unit: r.get(11),
            current_surah: r.get(12),
            current_unit: r.get(13),
        })
        .collect())
}

/// Rentang materi kurikulum (migrasi 57).
///
/// Dikemas jadi satu struct, bukan empat argumen angka berurutan: `(None,
/// Some(1), None, Some(20))` di tempat panggilan tak terbaca, dan menukar dua
/// di antaranya tetap lolos compiler.
#[derive(Debug, Clone, Copy, Default)]
pub struct CurriculumRange {
    /// Indeks surat 1-based (quran); None untuk hadist.
    pub start_surah: Option<i16>,
    /// Halaman (hadist) atau ayat (quran).
    pub start_unit: Option<i32>,
    pub end_surah: Option<i16>,
    pub end_unit: Option<i32>,
    /// Posisi berjalan (migrasi 59) — bukan bagian rentang, tapi diperiksa &
    /// disimpan bersamanya karena batasnya materi yang sama.
    pub current_surah: Option<i16>,
    pub current_unit: Option<i32>,
}

#[allow(clippy::too_many_arguments)]
pub async fn create_curriculum(
    pool: &Pool,
    class_id: i64,
    title: &str,
    book_id: Option<i64>,
    range: CurriculumRange,
) -> Result<i64> {
    let c = pool.get().await?;
    let order_index: i16 = c
        .query_one(
            // Postgres promotes MAX(smallint) + 1 → integer; cast balik ke
            // smallint di SQL (bukan hanya di sisi Rust) supaya tipe kolom
            // yang dibalik cocok dgn `.get::<_, i16>` — beda tipe = panic
            // runtime "error deserializing column" (bukan compile error,
            // krn tokio-postgres query berupa string biasa).
            "SELECT COALESCE(MAX(order_index) + 1, 0)::SMALLINT FROM curriculum WHERE class_id = $1",
            &[&class_id],
        )
        .await
        .context("create_curriculum: order_index")?
        .get(0);
    let row = c
        .query_one(
            "INSERT INTO curriculum \
                (class_id, title, order_index, book_id, \
                 start_surah, start_unit, end_surah, end_unit, current_surah, current_unit) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
            &[
                &class_id, &title, &order_index, &book_id,
                &range.start_surah, &range.start_unit, &range.end_surah, &range.end_unit,
                &range.current_surah, &range.current_unit,
            ],
        )
        .await
        .context("create_curriculum")?;
    Ok(row.get(0))
}

#[allow(clippy::too_many_arguments)]
pub async fn update_curriculum(
    pool: &Pool,
    id: i64,
    title: &str,
    book_id: Option<i64>,
    range: CurriculumRange,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE curriculum SET title = $2, book_id = $3, \
                start_surah = $4, start_unit = $5, end_surah = $6, end_unit = $7, \
                current_surah = $8, current_unit = $9, updated_at = NOW() \
             WHERE id = $1",
            &[
                &id, &title, &book_id,
                &range.start_surah, &range.start_unit, &range.end_surah, &range.end_unit,
                &range.current_surah, &range.current_unit,
            ],
        )
        .await
        .context("update_curriculum")?;
    Ok(n > 0)
}

pub async fn delete_curriculum(pool: &Pool, id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute("DELETE FROM curriculum WHERE id = $1", &[&id])
        .await
        .context("delete_curriculum")?;
    Ok(n > 0)
}

/// Update kolom rekaman sesi (dipanggil tiap chunk siaran — best effort).
pub async fn update_recording(
    pool: &Pool,
    session_id: i64,
    path: &str,
    mime: &str,
    size: i64,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE class_sessions SET recording_path = $2, recording_mime_type = $3, \
         recording_size = $4 WHERE id = $1",
        &[&session_id, &path, &mime, &size],
    )
    .await
    .context("update_recording")?;
    Ok(())
}

// ── Sisi SANTRI: kelas yang diikuti ──────────────────────────────────────────

/// Satu kelas yang diikuti seorang santri, lengkap dengan petugasnya.
pub struct SantriKelasRow {
    pub id: i64,
    pub name: String,
    pub category: Option<String>,
    pub jenjang: Option<String>,
    pub wali_kelas: Option<String>,
    pub pamong: Option<String>,
    /// Peran PEMIRSA di kelas ini (selalu false untuk santri).
    pub saya_wali: bool,
    pub saya_pamong: bool,
}

/// Kelas-kelas yang diikuti `user_id` (lewat `class_participants`).
///
/// DISTINCT: satu santri bisa terdaftar di kelas yang sama lewat lebih dari
/// satu jadwal (`class_participants.class_schedule_id`), dan tanpa ini kelasnya
/// muncul berulang di daftar santri.
pub async fn classes_of_student(pool: &Pool, user_id: i64) -> Result<Vec<SantriKelasRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT cl.id, cl.name, cl.category, cl.jenjang, \
                    w.full_name, pm.full_name \
             FROM class_participants cp \
             JOIN classes cl ON cl.id = cp.class_id \
             LEFT JOIN users w  ON w.id  = cl.wali_kelas_id \
             LEFT JOIN users pm ON pm.id = cl.pamong_id \
             WHERE cp.user_id = $1 \
             ORDER BY cl.name",
            &[&user_id],
        )
        .await
        .context("classes_of_student")?;
    Ok(rows
        .into_iter()
        .map(|r| SantriKelasRow {
            id: r.get(0),
            name: r.get(1),
            category: r.get(2),
            jenjang: r.get(3),
            wali_kelas: r.get(4),
            pamong: r.get(5),
            saya_wali: false,
            saya_pamong: false,
        })
        .collect())
}

/// Materi yang terdaftar di kurikulum sebuah KELAS.
///
/// Dipakai untuk membatasi pilihan materi jadwal & sesi: keduanya mengajarkan
/// apa yang direncanakan kelasnya, jadi menawarkan seluruh isi tabel `books`
/// membuka jalan mencatat kitab yang tak pernah masuk kurikulum — dan progres
/// kurikulumnya tak akan pernah bergerak.
pub async fn books_in_curriculum(
    pool: &Pool,
    class_id: i64,
) -> Result<Vec<super::books::BookRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT b.id, b.title, b.category, b.total_pages, b.surahs \
             FROM curriculum cu JOIN books b ON b.id = cu.book_id \
             WHERE cu.class_id = $1 ORDER BY b.title",
            &[&class_id],
        )
        .await
        .context("books_in_curriculum")?;
    Ok(rows
        .into_iter()
        .map(|r| super::books::BookRow {
            id: r.get(0),
            title: r.get(1),
            category: r.get(2),
            total_pages: r.get(3),
            surahs: r.get(4),
        })
        .collect())
}

/// Apakah materi `book_id` ada di kurikulum kelas pemilik SESI ini?
/// Pasangan [`schedule_book_in_curriculum`] untuk jalur sesi.
pub async fn session_book_in_curriculum(
    pool: &Pool,
    session_id: i64,
    book_id: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT 1 FROM class_sessions s \
               JOIN curriculum cu ON cu.class_id = s.class_id AND cu.book_id = $2 \
              WHERE s.id = $1 LIMIT 1",
            &[&session_id, &book_id],
        )
        .await
        .context("session_book_in_curriculum")?;
    Ok(row.is_some())
}

/// Kelas tempat `user_id` BERTUGAS — sebagai wali kelas, pamong, atau keduanya.
///
/// Pasangan [`classes_of_student`] untuk sisi staf. Satu query, bukan dua yang
/// digabung di Rust, supaya kelas yang ia pegang DUA peran sekaligus muncul
/// sekali saja dengan kedua penandanya.
pub async fn classes_of_staff(pool: &Pool, user_id: i64) -> Result<Vec<SantriKelasRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT cl.id, cl.name, cl.category, cl.jenjang, \
                    w.full_name, pm.full_name, \
                    (cl.wali_kelas_id = $1) AS saya_wali, \
                    (cl.pamong_id = $1) AS saya_pamong \
             FROM classes cl \
             LEFT JOIN users w  ON w.id  = cl.wali_kelas_id \
             LEFT JOIN users pm ON pm.id = cl.pamong_id \
             WHERE cl.wali_kelas_id = $1 OR cl.pamong_id = $1 \
             ORDER BY cl.name",
            &[&user_id],
        )
        .await
        .context("classes_of_staff")?;
    Ok(rows
        .into_iter()
        .map(|r| SantriKelasRow {
            id: r.get(0),
            name: r.get(1),
            category: r.get(2),
            jenjang: r.get(3),
            wali_kelas: r.get(4),
            pamong: r.get(5),
            saya_wali: r.get::<_, Option<bool>>(6).unwrap_or(false),
            saya_pamong: r.get::<_, Option<bool>>(7).unwrap_or(false),
        })
        .collect())
}

/// Mode verifikasi kelas pemilik sebuah SESI (migrasi 62):
/// "dua_tahap" | "guru" | "pamong". None = sesi tak ditemukan.
pub async fn session_verify_mode(pool: &Pool, session_id: i64) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cl.verify_mode FROM class_sessions cs \
               JOIN classes cl ON cl.id = cs.class_id WHERE cs.id = $1",
            &[&session_id],
        )
        .await
        .context("session_verify_mode")?;
    Ok(row.map(|r| r.get(0)))
}

// `set_class_verify_mode` dihapus: mode verifikasi kini ditulis bersama wali
// dan pamong dalam satu pernyataan di `set_class_staff`. Menyisakannya berarti
// menyediakan jalan kedua untuk mengubah rute verifikasi tanpa memeriksa
// syaratnya (mode ber-pamong wajib punya pamong) — dan syarat itu hanya ada di
// service::kelas::set_class_staff.
