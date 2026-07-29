//! repository/schedule.rs — Query class_schedules & class_sessions.

use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveTime};
use deadpool_postgres::Pool;

/// Satu sesi yang jatuh tempo pengingat WA pamong (~1 jam sebelum mulai).
pub struct DueReminder {
    pub session_id: i64,
    pub session_title: String,
    pub class_name: String,
    /// "HH:MM" jam mulai (dari jadwal).
    pub start_hm: String,
    pub pamong_phone: String,
    pub pamong_name: String,
    /// Nama dewan guru pengisi saat ini (None = belum diatur).
    pub teacher_name: Option<String>,
}

/// Klaim + tandai (idempotent) sesi yang mulai dalam ≤60 menit ke depan & belum
/// dinotifikasi, milik kelas ber-pamong (punya HP). UPDATE...RETURNING
/// menandai pamong_notified_at SEKALIGUS mengambil baris → aman dari kirim
/// ganda (race/loop). Sesi tanpa jadwal (tak ada jam) dilewati. Migrasi 30.
pub async fn claim_due_pamong_reminders(pool: &Pool) -> Result<Vec<DueReminder>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "UPDATE class_sessions s SET pamong_notified_at = NOW() \
             FROM class_schedules sch, classes cl, users pm \
             WHERE s.class_schedule_id = sch.id AND cl.id = s.class_id \
                AND pm.id = cl.pamong_id \
                AND s.status = 'scheduled' AND s.pamong_notified_at IS NULL \
                AND pm.phone_number IS NOT NULL AND pm.phone_number <> '' \
                AND (s.session_date + sch.start_time) AT TIME ZONE 'Asia/Jakarta' \
                    BETWEEN NOW() AND NOW() + INTERVAL '60 minutes' \
             RETURNING s.id, COALESCE(NULLIF(s.title, ''), cl.name), cl.name, \
                       to_char(sch.start_time, 'HH24:MI'), pm.phone_number, pm.full_name, \
                       (SELECT full_name FROM users t WHERE t.id = s.teacher_id)",
            &[],
        )
        .await
        .context("claim_due_pamong_reminders")?;
    Ok(rows
        .into_iter()
        .map(|r| DueReminder {
            session_id: r.get(0),
            session_title: r.get(1),
            class_name: r.get(2),
            start_hm: r.get(3),
            pamong_phone: r.get(4),
            pamong_name: r.get(5),
            teacher_name: r.get(6),
        })
        .collect())
}

pub struct ScheduleRow {
    pub title: Option<String>,
    pub class_name: String,
    pub start_time: NaiveTime,
}

/// Jadwal aktif terdekat milik santri (MVP: urut jam mulai).
pub async fn next_schedule(pool: &Pool, user_id: i64) -> Result<Option<ScheduleRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cs.title, c.name, cs.start_time \
             FROM class_participants cp \
             JOIN class_schedules cs ON cs.id = cp.class_schedule_id AND cs.status = 'active' \
             JOIN classes c ON c.id = cs.class_id \
             WHERE cp.user_id = $1 \
               AND cs.start_date <= CURRENT_DATE \
               AND (cs.end_date IS NULL OR cs.end_date >= CURRENT_DATE) \
             ORDER BY cs.start_time ASC LIMIT 1",
            &[&user_id],
        )
        .await
        .context("next_schedule")?;
    Ok(row.map(|r| ScheduleRow {
        title: r.get(0),
        class_name: r.get(1),
        start_time: r.get(2),
    }))
}

pub struct ActiveSchedule {
    pub id: i64,
    pub limit_entry: NaiveTime,
}

/// Jadwal aktif yang sedang berlangsung untuk user pada waktu WIB tertentu.
/// Jendela masuk: 45 menit sebelum start_time s/d end_time.
pub async fn active_schedule_now(
    pool: &Pool,
    user_id: i64,
    today: NaiveDate,
    now_time: NaiveTime,
) -> Result<Option<ActiveSchedule>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT cs.id, cs.limit_entery_time \
             FROM class_participants cp \
             JOIN class_schedules cs ON cs.id = cp.class_schedule_id AND cs.status = 'active' \
             WHERE cp.user_id = $1 \
               AND cs.start_date <= $2 AND (cs.end_date IS NULL OR cs.end_date >= $2) \
               AND $3::time >= cs.start_time - INTERVAL '45 minutes' \
               AND $3::time <= cs.end_time \
             ORDER BY cs.start_time LIMIT 1",
            &[&user_id, &today, &now_time],
        )
        .await
        .context("active_schedule_now")?;
    Ok(row.map(|r| ActiveSchedule {
        id: r.get(0),
        limit_entry: r.get(1),
    }))
}

/// Sesi kelas hari ini untuk jadwal tsb (bila guru sudah memulai sesi).
pub async fn session_for_schedule_today(
    pool: &Pool,
    schedule_id: i64,
    today: NaiveDate,
) -> Result<Option<i64>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT id FROM class_sessions \
             WHERE class_schedule_id = $1 AND session_date = $2 LIMIT 1",
            &[&schedule_id, &today],
        )
        .await?;
    Ok(row.map(|r| r.get(0)))
}

pub struct SessionRow {
    pub id: i64,
    pub title: Option<String>,
    pub class_name: String,
    pub session_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub status: String,
    pub teacher: Option<String>,
    pub teacher_id: Option<i64>,
    /// Pamong bertugas verifikasi sesi (migrasi 33) — None = pakai pamong kelas.
    pub pamong_id: Option<i64>,
    /// Kategori kelas (mis. "Pengajian", "Sholat") — menentukan boleh/tidaknya
    /// siaran suara direkam (lihat models::category_allows_recording).
    pub category: Option<String>,
}

const SESSION_COLS: &str = "SELECT s.id, COALESCE(s.title, cs.title), c.name, s.session_date, \
     cs.start_time, s.status, t.full_name, s.teacher_id, COALESCE(cs.category, c.category), \
     s.pamong_id \
     FROM class_sessions s \
     JOIN classes c ON c.id = s.class_id \
     LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
     LEFT JOIN users t ON t.id = s.teacher_id";

fn row_to_session(r: tokio_postgres::Row) -> SessionRow {
    SessionRow {
        id: r.get(0),
        title: r.get(1),
        class_name: r.get(2),
        session_date: r.get(3),
        start_time: r.get(4),
        status: r.get(5),
        teacher: r.get(6),
        teacher_id: r.get(7),
        category: r.get(8),
        pamong_id: r.get(9),
    }
}

/// SEMUA sesi (admin/pamong/dewan guru) dalam rentang [since, until] — untuk
/// halaman /sesi yang menampilkan "1 minggu terakhir yang sudah lewat".
pub async fn all_sessions(
    pool: &Pool,
    since: chrono::NaiveDate,
    until: chrono::NaiveDate,
    limit: i64,
) -> Result<Vec<SessionRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "{SESSION_COLS} WHERE s.session_date >= $1 AND s.session_date <= $2 \
         ORDER BY s.session_date DESC, s.id DESC LIMIT $3"
    );
    let rows = c.query(&sql, &[&since, &until, &limit]).await.context("all_sessions")?;
    Ok(rows.into_iter().map(row_to_session).collect())
}


/// Sesi kelas-kelas yang DIIKUTI santri ini saja, dalam rentang [since, until].
pub async fn sessions_for_student(
    pool: &Pool,
    user_id: i64,
    since: chrono::NaiveDate,
    until: chrono::NaiveDate,
    limit: i64,
) -> Result<Vec<SessionRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "{SESSION_COLS} \
         WHERE s.class_id IN (SELECT class_id FROM class_participants WHERE user_id = $1) \
           AND s.session_date >= $2 AND s.session_date <= $3 \
         ORDER BY s.session_date DESC, s.id DESC LIMIT $4"
    );
    let rows = c
        .query(&sql, &[&user_id, &since, &until, &limit])
        .await
        .context("sessions_for_student")?;
    Ok(rows.into_iter().map(row_to_session).collect())
}

/// Sesi kelas-kelas yang diikuti ANAK-ANAK terhubung dari satu orang tua,
/// dalam rentang [since, until] (kalender akademik sisi ortu).
pub async fn sessions_for_parent(
    pool: &Pool,
    parent_id: i64,
    since: chrono::NaiveDate,
    until: chrono::NaiveDate,
    limit: i64,
) -> Result<Vec<SessionRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "{SESSION_COLS} \
         WHERE s.class_id IN ( \
             SELECT cp.class_id FROM class_participants cp \
             JOIN parent_connections pc ON pc.student_id = cp.user_id \
                  AND pc.parent_id = $1 AND pc.status = 'connected') \
           AND s.session_date >= $2 AND s.session_date <= $3 \
         ORDER BY s.session_date ASC, s.id ASC LIMIT $4"
    );
    let rows = c
        .query(&sql, &[&parent_id, &since, &until, &limit])
        .await
        .context("sessions_for_parent")?;
    Ok(rows.into_iter().map(row_to_session).collect())
}

/// Sesi milik satu kelas (untuk halaman detail kelas).
/// Sesi kelas MULAI hari ini (`from`) ke depan — sesi yang sudah lewat TIDAK
/// ditampilkan. Urut menaik (terdekat dulu). `from` = tanggal WIB dari service.
pub async fn sessions_of_class(
    pool: &Pool,
    class_id: i64,
    from: chrono::NaiveDate,
    limit: i64,
) -> Result<Vec<SessionRow>> {
    let c = pool.get().await?;
    let sql = format!(
        "{SESSION_COLS} WHERE s.class_id = $1 AND s.session_date >= $2 \
         ORDER BY s.session_date ASC, s.id ASC LIMIT $3"
    );
    let rows = c
        .query(&sql, &[&class_id, &from, &limit])
        .await
        .context("sessions_of_class")?;
    Ok(rows.into_iter().map(row_to_session).collect())
}

// ── Detail sesi (staf): info + absensi + chat + rekaman ──────────────────────

pub struct SessionDetailRow {
    pub id: i64,
    pub class_id: i64,
    pub title: Option<String>,
    pub class_name: String,
    pub session_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    /// Jam selesai jadwal (None bila sesi ad-hoc tanpa jadwal terpasang) —
    /// dipakai memvalidasi jendela "Mulai Sesi" (mulai-10m s/d selesai+10m).
    pub end_time: Option<NaiveTime>,
    pub status: String,
    pub teacher: Option<String>,
    pub recording_path: Option<String>,
    pub recording_size: Option<i64>,
    pub teacher_id: Option<i64>,
    /// Pamong bertugas verifikasi sesi (migrasi 33) — None = pakai pamong kelas.
    pub pamong_id: Option<i64>,
    pub category: Option<String>,
    /// Materi buku sesi ini (migrasi 20) — None bila tak ada buku dipilih.
    pub book_id: Option<i64>,
    pub book_title: Option<String>,
    pub book_pages: serde_json::Value,
}

/// Kategori kelas dari sebuah sesi — query ringan (dipakai guard server-side
/// tiap potongan siaran suara di web/live_audio.rs, bukan seluruh detail).
pub async fn session_category(pool: &Pool, session_id: i64) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT COALESCE(cs.category, c.category) \
             FROM class_sessions s JOIN classes c ON c.id = s.class_id \
             LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
             WHERE s.id = $1",
            &[&session_id],
        )
        .await
        .context("session_category")?;
    Ok(row.and_then(|r| r.get(0)))
}

pub async fn session_detail(pool: &Pool, id: i64) -> Result<Option<SessionDetailRow>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT s.id, s.class_id, COALESCE(s.title, cs.title), c.name, s.session_date, \
                    cs.start_time, cs.end_time, s.status, t.full_name, s.recording_path, \
                    s.recording_size, s.teacher_id, COALESCE(cs.category, c.category), \
                    s.book_id, b.title, s.book_pages, s.pamong_id \
             FROM class_sessions s \
             JOIN classes c ON c.id = s.class_id \
             LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
             LEFT JOIN users t ON t.id = s.teacher_id \
             LEFT JOIN books b ON b.id = s.book_id \
             WHERE s.id = $1",
            &[&id],
        )
        .await
        .context("session_detail")?;
    Ok(row.map(|r| SessionDetailRow {
        id: r.get(0),
        class_id: r.get(1),
        title: r.get(2),
        class_name: r.get(3),
        session_date: r.get(4),
        start_time: r.get(5),
        end_time: r.get(6),
        status: r.get(7),
        teacher: r.get(8),
        recording_path: r.get(9),
        recording_size: r.get(10),
        teacher_id: r.get(11),
        category: r.get(12),
        book_id: r.get(13),
        book_title: r.get(14),
        book_pages: r.get(15),
        pamong_id: r.get(16),
    }))
}

/// Anggota kelas + status absensinya PADA sesi ini (NULL = belum tercatat).
pub async fn session_attendance(
    pool: &Pool,
    session_id: i64,
    class_id: i64,
) -> Result<Vec<(i64, String, Option<String>, Option<String>, Option<chrono::DateTime<chrono::Utc>>)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, a.status, a.scanned_at \
             FROM (SELECT DISTINCT user_id FROM class_participants WHERE class_id = $2) cp \
             JOIN users u ON u.id = cp.user_id AND u.role IN ('santri', 'santri_finance') \
             LEFT JOIN attendances a ON a.user_id = u.id AND a.class_session_id = $1 \
             ORDER BY u.full_name",
            &[&session_id, &class_id],
        )
        .await
        .context("session_attendance")?;
    Ok(rows
        .into_iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3), r.get(4)))
        .collect())
}

/// Transkrip chat sesi (pesan terhapus disembunyikan), urut waktu.
pub async fn session_chats(
    pool: &Pool,
    session_id: i64,
    limit: i64,
) -> Result<Vec<(String, String, chrono::DateTime<chrono::Utc>)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.full_name, ch.message, ch.created_at \
             FROM class_session_chats ch JOIN users u ON u.id = ch.user_id \
             WHERE ch.session_id = $1 AND ch.is_deleted = FALSE \
             ORDER BY ch.created_at ASC LIMIT $2",
            &[&session_id, &limit],
        )
        .await
        .context("session_chats")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1), r.get(2))).collect())
}

/// Tandai HADIR manual oleh staf (method='manual', gate 'manual'). Idempotent
/// lewat UNIQUE(user_id, class_session_id) → sudah tercatat = tak diubah.
/// Masuk antrean verifikasi normal (pamong_status/verify_status 'pending').
pub async fn mark_manual_present(
    pool: &Pool,
    student_id: i64,
    session_id: i64,
) -> Result<bool> {
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO attendances \
                (user_id, class_session_id, class_schedule_id, gate_label, status, method, note) \
             SELECT $1, s.id, s.class_schedule_id, 'manual', 'present', 'manual', 'ditandai staf' \
             FROM class_sessions s WHERE s.id = $2 \
             ON CONFLICT (user_id, class_session_id) DO NOTHING",
            &[&student_id, &session_id],
        )
        .await
        .context("mark_manual_present")?;
    Ok(n > 0)
}

/// Tandai BANYAK santri sekaligus pada sesi dgn `status` ('present'|'absent').
/// Set-based via unnest; ON CONFLICT skip yang sudah tercatat. Masuk antrean
/// verifikasi normal (pamong/verify 'pending' → poin di tahap final, migrasi 33).
/// Return jumlah BARU tercatat.
pub async fn mark_attendance_bulk(
    pool: &Pool,
    session_id: i64,
    student_ids: &[i64],
    status: &str,
) -> Result<i64> {
    if student_ids.is_empty() {
        return Ok(0);
    }
    let note = if status == "absent" { "dialpakan staf" } else { "ditandai staf" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "INSERT INTO attendances \
                (user_id, class_session_id, class_schedule_id, gate_label, status, method, note) \
             SELECT uid, s.id, s.class_schedule_id, 'manual', $3, 'manual', $4 \
             FROM class_sessions s CROSS JOIN unnest($2::bigint[]) AS uid \
             WHERE s.id = $1 \
             ON CONFLICT (user_id, class_session_id) DO NOTHING",
            &[&session_id, &student_ids, &status, &note],
        )
        .await
        .context("mark_attendance_bulk")?;
    Ok(n as i64)
}

/// Kirim satu pesan chat sesi.
pub async fn insert_session_chat(
    pool: &Pool,
    session_id: i64,
    user_id: i64,
    message: &str,
) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "INSERT INTO class_session_chats (session_id, user_id, message) VALUES ($1, $2, $3)",
        &[&session_id, &user_id, &message],
    )
    .await
    .context("insert_session_chat")?;
    Ok(())
}

/// Apakah user peserta kelas ini? (akses santri ke ruang sesi live)
pub async fn is_class_participant(pool: &Pool, class_id: i64, user_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM class_participants WHERE class_id = $1 AND user_id = $2)",
            &[&class_id, &user_id],
        )
        .await
        .context("is_class_participant")?;
    Ok(row.get(0))
}

/// Jumlah peserta unik sebuah kelas (header ruang live).
pub async fn class_member_count(pool: &Pool, class_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(DISTINCT user_id) FROM class_participants WHERE class_id = $1",
            &[&class_id],
        )
        .await
        .context("class_member_count")?;
    Ok(row.get(0))
}
