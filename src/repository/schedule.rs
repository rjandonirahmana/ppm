//! repository/schedule.rs — Query class_schedules & class_sessions.

use anyhow::{Context, Result};
use chrono::{NaiveDate, NaiveTime};
use deadpool_postgres::Pool;

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
            // Dibaca dari SESI, bukan aturan jadwal: "jadwal berikutnya" adalah
            // kejadian nyata. Versi lama memilih jadwal mana pun yang tanggal
            // berlakunya mencakup hari ini, sehingga beranda santri bisa
            // menjanjikan "Tahfidz 04:30" pada hari yang tak ada Tahfidz-nya.
            //
            // Tanggal dibandingkan dalam WIB, bukan CURRENT_DATE: server
            // berjalan UTC, dan sampai pukul 07:00 WIB CURRENT_DATE masih
            // menunjuk hari kemarin — sesi subuh hari ini akan terlewat.
            // JOIN (bukan LEFT JOIN) ke class_schedules: yang dicari adalah
            // jadwal, dan sesi dadakan tanpa jadwal memang tak punya jam mulai
            // untuk ditampilkan.
            "SELECT COALESCE(NULLIF(ses.title, ''), sch.title, ''), c.name, sch.start_time \
             FROM class_participants cp \
             JOIN class_sessions ses ON ses.class_id = cp.class_id \
                  AND ses.status <> 'cancelled' \
             JOIN class_schedules sch ON sch.id = ses.class_schedule_id \
             JOIN classes c ON c.id = ses.class_id \
             WHERE cp.user_id = $1 \
               AND (ses.session_date, sch.end_time) \
                   >= ((NOW() AT TIME ZONE 'Asia/Jakarta')::date, \
                       (NOW() AT TIME ZONE 'Asia/Jakarta')::time) \
             ORDER BY ses.session_date, sch.start_time, ses.id LIMIT 1",
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
    /// Sesi hari ini milik jadwal itu. Selalu ada: keberadaannyalah yang
    /// membuktikan jadwal ini memang berlangsung hari ini.
    pub session_id: i64,
}

/// Jadwal aktif yang sedang berlangsung untuk user pada waktu WIB tertentu,
/// DI PERANGKAT tempat kartu ditempel. Jendela masuk: 45 menit sebelum
/// start_time s/d end_time.
///
/// Aturan ruang (`class_schedules.room_id`):
///   • room_id TERISI → jadwal itu hanya cocok bila di-tap di perangkat itu.
///     Santri yang mestinya di masjid lalu menempel kartu di gedung putra TIDAK
///     terhitung hadir — tapnya jatuh jadi `outside_schedule`.
///   • room_id NULL   → jadwal bebas di-tap di perangkat mana pun.
///
/// Bila dua jadwal sama-sama cocok, yang TERIKAT ruang ini didahulukan atas
/// yang bebas-ruang — tap di ruang tertentu lebih spesifik, jadi lebih mungkin
/// itulah maksud santrinya.
pub async fn active_schedule_now(
    pool: &Pool,
    user_id: i64,
    today: NaiveDate,
    now_time: NaiveTime,
    device_id: i64,
) -> Result<Option<ActiveSchedule>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            // JOIN class_sessions ITU INTINYA — bukan sekadar mengambil id sesi.
            //
            // `class_schedules` adalah ATURAN ("Tahfidz tiap Senin 04:30"),
            // bukan kejadian. Versi lama query ini hanya menguji rentang
            // start_date..end_date + jam, jadi jadwal Senin ikut cocok pada
            // Selasa, Rabu, dan seterusnya: santri bisa menempel kartu tiap
            // hari dan tercatat hadir di kelas yang tak berlangsung.
            //
            // Pola recurrence-nya sendiri sudah dihitung di satu tempat
            // (service::kelas::dates_in_range) dan dimaterialisasi jadi baris
            // `class_sessions` oleh tugas latar. Menyalin ulang logika
            // daily/weekly/monthly/custom ke SQL di sini berarti dua penafsiran
            // recurrence yang harus terus sepakat. Jadi: yang berhak dihadiri
            // hanya yang punya SESI hari ini.
            //
            // Sesi 'cancelled' (ditandai libur) tak menerima absensi — itu
            // justru gunanya menandai libur.
            "SELECT sch.id, sch.limit_entery_time, ses.id \
             FROM class_participants cp \
             JOIN class_schedules sch ON sch.class_id = cp.class_id AND sch.status = 'active' \
             JOIN class_sessions ses ON ses.class_schedule_id = sch.id \
                  AND ses.session_date = $2 AND ses.status <> 'cancelled' \
             WHERE cp.user_id = $1 \
               AND $3::time >= sch.start_time - INTERVAL '45 minutes' \
               AND $3::time <= sch.end_time \
               AND (sch.room_id IS NULL OR sch.room_id = $4) \
             ORDER BY (sch.room_id IS NULL), sch.start_time, ses.id LIMIT 1",
            &[&user_id, &today, &now_time, &device_id],
        )
        .await
        .context("active_schedule_now")?;
    Ok(row.map(|r| ActiveSchedule {
        id: r.get(0),
        limit_entry: r.get(1),
        session_id: r.get(2),
    }))
}

/// Nama ruang tempat santri SEHARUSNYA berada saat ini, bila jadwalnya terikat
/// perangkat LAIN. None = memang tak ada jadwal aktif.
///
/// Dipakai hanya di jalur gagal (tap tak cocok jadwal) untuk memberi pesan yang
/// menolong — "kelasmu di Masjid, bukan di sini" jauh lebih berguna bagi santri
/// yang berdiri di depan pembaca kartu daripada sekadar "di luar jadwal".
pub async fn active_schedule_room_elsewhere(
    pool: &Pool,
    user_id: i64,
    today: NaiveDate,
    now_time: NaiveTime,
    device_id: i64,
) -> Result<Option<String>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            // Sama seperti active_schedule_now: hanya jadwal yang PUNYA SESI
            // hari ini yang boleh bicara. Tanpa ini pesan "kelasmu di Masjid"
            // bisa muncul pada hari yang kelas itu tak berlangsung sama sekali.
            "SELECT COALESCE(dev.location, dev.device_name) \
             FROM class_participants cp \
             JOIN class_schedules sch ON sch.class_id = cp.class_id AND sch.status = 'active' \
             JOIN class_sessions ses ON ses.class_schedule_id = sch.id \
                  AND ses.session_date = $2 AND ses.status <> 'cancelled' \
             JOIN rfid_devices dev ON dev.id = sch.room_id \
             WHERE cp.user_id = $1 \
               AND $3::time >= sch.start_time - INTERVAL '45 minutes' \
               AND $3::time <= sch.end_time \
               AND sch.room_id <> $4 \
             ORDER BY sch.start_time LIMIT 1",
            &[&user_id, &today, &now_time, &device_id],
        )
        .await
        .context("active_schedule_room_elsewhere")?;
    Ok(row.map(|r| r.get(0)))
}

// `session_for_schedule_today` dihapus: `active_schedule_now` kini menempuh
// class_sessions untuk membuktikan jadwalnya memang berlangsung hari ini, jadi
// id sesinya sudah ikut terbawa dan pencarian kedua tinggal duplikasi.

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
         ORDER BY s.session_date DESC, cs.start_time ASC NULLS LAST, s.id ASC LIMIT $3"
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
         ORDER BY s.session_date DESC, cs.start_time ASC NULLS LAST, s.id ASC LIMIT $4"
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
         ORDER BY s.session_date ASC, cs.start_time ASC NULLS LAST, s.id ASC LIMIT $4"
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
         ORDER BY s.session_date ASC, cs.start_time ASC NULLS LAST, s.id ASC LIMIT $3"
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
    /// Wali & pamong KELAS — cadangan bila sesi belum menetapkan petugasnya.
    /// Dipakai menghitung siapa yang boleh mengoreksi absensi (migrasi 51),
    /// mencerminkan COALESCE di repository::correct_attendance.
    pub class_wali_id: Option<i64>,
    pub class_pamong_id: Option<i64>,
    /// Nama wali kelas — CADANGAN tampilan bila sesi tak punya guru pengisi.
    pub wali_name: Option<String>,
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
    /// Label kategori untuk TAMPILAN — bisa teks bebas dari jadwal.
    pub category: Option<String>,
    /// Kategori KELAS-nya: kbm | bacaan | non_kbm (migrasi 65). Ini yang jadi
    /// gerbang fitur, bukan `category` di atas — kategori jadwal boleh diketik
    /// apa saja ("Pengajian KBM Malam"), jadi memakainya sebagai penentu
    /// membuat panel Hafalan muncul di kelas yang bukan Bacaan.
    pub class_category: String,
    /// Materi buku sesi ini (migrasi 20) — None bila tak ada buku dipilih.
    pub book_id: Option<i64>,
    pub book_title: Option<String>,
    pub book_pages: serde_json::Value,
    /// Materi TARGET/rencana (migrasi 41).
    pub target_book_id: Option<i64>,
    pub target_book_title: Option<String>,
    pub target_pages: serde_json::Value,
    /// Catatan ayat/hadith aktual (migrasi 41).
    pub actual_detail: String,
}

/// Kategori kelas dari sebuah sesi — query ringan (dipakai guard server-side
/// tiap potongan siaran suara di web/live_audio.rs, bukan seluruh detail).
/// Siapa saja yang BERHAK menyiarkan sesi ini: (pengisi, pamong sesi/kelas,
/// wali kelas). Dipakai `web/live_audio.rs::post_chunk` untuk menolak staf lain
/// menimpa rekaman sesi yang bukan urusannya.
pub async fn session_broadcasters(
    pool: &Pool,
    session_id: i64,
) -> Result<Option<(Option<i64>, Option<i64>, Option<i64>, String)>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            // `status` ikut: siaran tak boleh dimulai pada sesi yang sudah
            // SELESAI. Setelah selesai, rekamannya dipindah ke penyimpanan
            // objek dan berkas lokalnya dihapus — potongan yang datang
            // belakangan akan membuat berkas lokal BARU yang tak seorang pun
            // tahu keberadaannya, sementara DB menunjuk berkas final.
            "SELECT s.teacher_id, COALESCE(s.pamong_id, cl.pamong_id), cl.wali_kelas_id, s.status \
             FROM class_sessions s JOIN classes cl ON cl.id = s.class_id \
             WHERE s.id = $1",
            &[&session_id],
        )
        .await
        .context("session_broadcasters")?;
    Ok(row.map(|r| (r.get(0), r.get(1), r.get(2), r.get(3))))
}

/// Satu sesi KBM yang sebentar lagi mulai dan pamongnya perlu diingatkan.
pub struct PengingatSesi {
    pub session_id: i64,
    pub class_name: String,
    pub title: String,
    /// "05:00 – 06:30"
    pub jam: String,
    pub pamong_name: String,
    pub pamong_phone: String,
    /// Guru sesi sudah ditunjuk? Pesan menyesuaikan — mengingatkan hal yang
    /// sudah dikerjakan hanya melatih orang mengabaikan pesan berikutnya.
    pub ada_guru: bool,
    pub ada_pamong_sesi: bool,
}

/// Sesi KBM yang mulai ~1 jam lagi, pamong kelasnya punya nomor HP, dan
/// pengingatnya belum pernah dikirim (migrasi 67).
///
/// Jendelanya `[+{dari} menit, +{sampai} menit]` dari sekarang WIB — dilebarkan
/// melebihi jarak antar-tick supaya tak ada sesi yang terlewat di celah waktu,
/// sementara `pamong_reminded_at` yang menjaga tak ada yang dikirim dua kali.
///
/// Hanya KBM: kelas lain diverifikasi pamong bertugas satu langkah, tak ada
/// guru pengajar yang perlu ditunjuk lebih dulu.
pub async fn sesi_perlu_pengingat(
    pool: &Pool,
    dari_menit: i32,
    sampai_menit: i32,
) -> Result<Vec<PengingatSesi>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT s.id, cl.name, COALESCE(NULLIF(s.title, ''), sch.title, 'Sesi Kelas'), \
                    to_char(sch.start_time, 'HH24:MI') || ' – ' || to_char(sch.end_time, 'HH24:MI'), \
                    pm.full_name, pm.phone_number, \
                    s.teacher_id IS NOT NULL, s.pamong_id IS NOT NULL \
             FROM class_sessions s \
             JOIN classes cl ON cl.id = s.class_id AND cl.category = 'kbm' \
             JOIN class_schedules sch ON sch.id = s.class_schedule_id \
             JOIN users pm ON pm.id = cl.pamong_id \
                  AND pm.is_active AND COALESCE(pm.phone_number, '') <> '' \
             WHERE s.status <> 'cancelled' \
               AND s.pamong_reminded_at IS NULL \
               AND s.session_date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date \
               AND sch.start_time BETWEEN \
                     ((NOW() AT TIME ZONE 'Asia/Jakarta') + make_interval(mins => $1))::time \
                 AND ((NOW() AT TIME ZONE 'Asia/Jakarta') + make_interval(mins => $2))::time \
             ORDER BY sch.start_time",
            &[&dari_menit, &sampai_menit],
        )
        .await
        .context("sesi_perlu_pengingat")?;
    Ok(rows
        .into_iter()
        .map(|r| PengingatSesi {
            session_id: r.get(0),
            class_name: r.get(1),
            title: r.get(2),
            jam: r.get(3),
            pamong_name: r.get(4),
            pamong_phone: r.get::<_, Option<String>>(5).unwrap_or_default(),
            ada_guru: r.get(6),
            ada_pamong_sesi: r.get(7),
        })
        .collect())
}

/// Tandai pengingat sesi sudah terkirim. Dipanggil SETELAH WA berhasil dikirim
/// — bila ditandai lebih dulu lalu pengirimannya gagal, pamongnya tak akan
/// pernah diingatkan sama sekali.
pub async fn tandai_pengingat_terkirim(pool: &Pool, session_id: i64) -> Result<()> {
    let c = pool.get().await?;
    c.execute(
        "UPDATE class_sessions SET pamong_reminded_at = NOW() WHERE id = $1",
        &[&session_id],
    )
    .await
    .context("tandai_pengingat_terkirim")?;
    Ok(())
}

/// Apakah `user_id` berkepentingan atas sesi ini?
///
/// SATU definisi untuk semua pintu sesi (siaran, unduh rekaman, SSE, chat).
/// Sebelumnya tiap endpoint menafsirkan sendiri — dan `get_data`/`download`
/// menafsirkannya sebagai "punya token yang sah", yang berarti santri mana pun
/// cukup menebak id sesi untuk mendengarkan rekaman kelas lain.
///
/// Yang berkepentingan: petugas sesi itu (guru/pamong), petugas kelasnya (wali
/// kelas/pamong kelas), santri anggota kelasnya, dan orang tua yang terhubung
/// dengan salah satu anggotanya. Peran pengawas lintas-kelas (admin, ketua,
/// dewan guru) TIDAK diurus di sini — itu keputusan peran, bukan keterkaitan
/// data, dan pemanggilnya yang menentukan (lihat `web::live_audio`).
///
/// `false` juga berarti "sesi tak ada" — pemanggil tak boleh membedakan
/// keduanya, karena selisih jawaban itu sendiri membocorkan sesi mana yang ada.
pub async fn session_stakeholder(pool: &Pool, session_id: i64, user_id: i64) -> Result<bool> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT EXISTS ( \
                SELECT 1 FROM class_sessions s \
                  JOIN classes cl ON cl.id = s.class_id \
                 WHERE s.id = $1 AND ( \
                       s.teacher_id = $2 OR s.pamong_id = $2 \
                    OR cl.wali_kelas_id = $2 OR cl.pamong_id = $2 \
                    OR EXISTS (SELECT 1 FROM class_participants cp \
                                WHERE cp.class_id = s.class_id AND cp.user_id = $2) \
                    OR EXISTS (SELECT 1 FROM parent_connections pc \
                                 JOIN class_participants cp2 ON cp2.user_id = pc.student_id \
                                WHERE pc.parent_id = $2 AND pc.status = 'connected' \
                                  AND cp2.class_id = s.class_id) \
                 ))",
            &[&session_id, &user_id],
        )
        .await
        .context("session_stakeholder")?;
    Ok(row.get(0))
}

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
                    s.book_id, b.title, s.book_pages, s.pamong_id, \
                    s.target_book_id, tb.title, s.target_pages, s.actual_detail, \
                    c.wali_kelas_id, c.pamong_id, w.full_name, c.category \
             FROM class_sessions s \
             JOIN classes c ON c.id = s.class_id \
             LEFT JOIN class_schedules cs ON cs.id = s.class_schedule_id \
             LEFT JOIN users t ON t.id = s.teacher_id \
             LEFT JOIN users w ON w.id = c.wali_kelas_id \
             LEFT JOIN books b ON b.id = s.book_id \
             LEFT JOIN books tb ON tb.id = s.target_book_id \
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
        target_book_id: r.get(17),
        target_book_title: r.get(18),
        target_pages: r.get(19),
        actual_detail: r.get(20),
        class_wali_id: r.get(21),
        class_pamong_id: r.get(22),
        wali_name: r.get(23),
        class_category: r.get(24),
    }))
}

/// Anggota kelas + status absensinya PADA sesi ini (NULL = belum tercatat).
/// Satu baris absensi sesi. Struct bernama, bukan tuple: dulu 5 elemen tanpa
/// nama dan tiap penambahan kolom memaksa pembacanya menghitung posisi.
pub struct SessionAttRaw {
    pub user_id: i64,
    pub full_name: String,
    pub nis: Option<String>,
    /// None = santri terdaftar di kelas tapi belum ada catatan absensi.
    pub status: Option<String>,
    pub scanned_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Id baris absensi — dibutuhkan untuk KOREKSI (migrasi 51). None = belum
    /// ada barisnya, jadi tak ada yang bisa dikoreksi.
    pub att_id: Option<i64>,
}

pub async fn session_attendance(
    pool: &Pool,
    session_id: i64,
    class_id: i64,
) -> Result<Vec<SessionAttRaw>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT u.id, u.full_name, u.nis, a.status, a.scanned_at, a.id \
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
        .map(|r| SessionAttRaw {
            user_id: r.get(0),
            full_name: r.get(1),
            nis: r.get(2),
            status: r.get(3),
            scanned_at: r.get(4),
            att_id: r.get(5),
        })
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
            // JOIN class_participants = pagarnya. UI memang hanya menampilkan
            // santri kelas ini, tapi UI bukan batas keamanan: request bisa
            // dirakit sendiri dengan user_id siapa pun. Tanpa join ini, satu
            // request cukup untuk menempelkan kehadiran (dan poinnya) pada
            // orang yang tak pernah masuk kelas itu.
            "INSERT INTO attendances \
                (user_id, class_session_id, class_schedule_id, gate_label, status, method, note, \
                 scan_date) \
             SELECT cp.user_id, s.id, s.class_schedule_id, 'manual', 'present', 'manual', \
                    'ditandai staf', s.session_date \
             FROM class_sessions s \
             JOIN class_participants cp ON cp.class_id = s.class_id AND cp.user_id = $1 \
             WHERE s.id = $2 \
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
                (user_id, class_session_id, class_schedule_id, gate_label, status, method, note, \
                 scan_date) \
             // unnest disaring lewat class_participants — id yang bukan anggota
             // kelas sesi ini diam-diam dijatuhkan, bukan dicatat (alasan sama
             // dengan mark_manual_present).
             SELECT cp.user_id, s.id, s.class_schedule_id, 'manual', $3, 'manual', $4, \
                    s.session_date \
             FROM class_sessions s \
             JOIN class_participants cp ON cp.class_id = s.class_id \
             WHERE s.id = $1 AND cp.user_id = ANY($2::bigint[]) \
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

/// Santri sebuah kelas + nomor HP-nya (untuk broadcast jadwal via WhatsApp).
/// Distinct per santri; HP None/'' = santri belum punya nomor (dilewati di
/// service). Hanya peran santri (bukan staf yang kebetulan ikut kelas).
pub async fn class_student_contacts(
    pool: &Pool,
    class_id: i64,
) -> Result<Vec<(String, Option<String>)>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT u.full_name, u.phone_number \
             FROM class_participants cp \
             JOIN users u ON u.id = cp.user_id AND u.role IN ('santri', 'santri_finance') \
             WHERE cp.class_id = $1 \
             ORDER BY u.full_name",
            &[&class_id],
        )
        .await
        .context("class_student_contacts")?;
    Ok(rows.into_iter().map(|r| (r.get(0), r.get(1))).collect())
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
