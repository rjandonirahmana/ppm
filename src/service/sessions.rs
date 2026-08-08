//! service/sessions.rs — Daftar sesi kelas per-peran.
//! Santri → hanya sesi kelas yang diikutinya; admin/pamong/dewan guru → SEMUA
//! (nantinya bisa mengelola/update sesi dari halaman yang sama).

use anyhow::Result;
use deadpool_postgres::Pool;

use chrono::{Duration, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use super::fmt::{fmt_date, wib};
use crate::models::{category_allows_recording, SessionItem, SessionsData, SessionUser};
use crate::repository as repo;

// CATATAN: `send_pamong_session_reminders` (migrasi 30) DIHAPUS di sini.
// Pengingat WA pamong sekarang satu jalur saja: `permits::ingatkan_pamong_sesi`
// (migrasi 67). Sebelumnya keduanya hidup berdampingan dan berjalan dari dua
// task berkala yang tak saling tahu — satu pamong menerima dua WhatsApp untuk
// satu sesi. Lihat main.rs dan migrasi 70.

/// Jendela boleh MENGAKTIFKAN sesi (mulai/mulai-ulang): 10 menit sebelum jam
/// mulai jadwal s/d 10 menit sesudah jam selesai jadwal (permintaan user).
/// Sesi ad-hoc tanpa jadwal (start/end None) TIDAK dibatasi — tak ada acuan jam.
fn start_window_reason(
    date: NaiveDate,
    start: Option<NaiveTime>,
    end: Option<NaiveTime>,
) -> Option<String> {
    let (Some(start), Some(end)) = (start, end) else {
        return None;
    };
    let window_start = NaiveDateTime::new(date, start) - Duration::minutes(10);
    let window_end = NaiveDateTime::new(date, end) + Duration::minutes(10);
    let now = Utc::now().with_timezone(&wib()).naive_local();
    if now < window_start {
        Some(format!(
            "Sesi baru bisa dimulai mulai {} {} WIB (10 menit sebelum jadwal).",
            fmt_date(window_start.date()),
            window_start.format("%H:%M"),
        ))
    } else if now > window_end {
        Some(format!(
            "Jendela mulai sesi sudah lewat ({} {} WIB).",
            fmt_date(window_end.date()),
            window_end.format("%H:%M"),
        ))
    } else {
        None
    }
}

fn status_display(status: &str) -> (&'static str, &'static str) {
    match status {
        "ongoing" => ("Berlangsung", "ongoing"),
        "finished" => ("Selesai", "finished"),
        "cancelled" => ("Dibatalkan", "cancelled"),
        _ => ("Terjadwal", "scheduled"),
    }
}

pub async fn list_for(pool: &Pool, user: &SessionUser) -> Result<SessionsData> {
    // Dua daftar (tab): TERJADWAL = hari ini s/d +7 hari; SUDAH LEWAT =
    // kemarin s/d −7 hari. Keduanya urut tanggal DESC.
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let since = today - Duration::days(7);
    let until = today + Duration::days(7);
    let yesterday = today - Duration::days(1);
    // Guru & dewan guru = SATU entitas: sama-sama lihat & kelola SEMUA sesi.
    let all_scope = crate::models::role_satisfies(&user.role, &["admin", "supervisor", "dewan_guru", "teacher"]);
    let (upcoming_rows, past_rows) = if all_scope {
        tokio::join!(
            repo::all_sessions(pool, today, until, 100),
            repo::all_sessions(pool, since, yesterday, 100),
        )
    } else {
        tokio::join!(
            repo::sessions_for_student(pool, user.id, today, until, 100),
            repo::sessions_for_student(pool, user.id, since, yesterday, 100),
        )
    };

    let to_items = |rows: Vec<repo::SessionRow>| -> Vec<SessionItem> {
        rows.into_iter()
            .map(|r| {
                let (status_label, status_kind) = status_display(&r.status);
                SessionItem {
                    id: r.id,
                    title: r.title.unwrap_or_else(|| r.class_name.clone()),
                    class_name: r.class_name,
                    date_label: fmt_date(r.session_date),
                    time_label: r
                        .start_time
                        .map(|t| format!("{} WIB", t.format("%H:%M")))
                        .unwrap_or_else(|| "-".into()),
                    status_label: status_label.into(),
                    status_kind: status_kind.into(),
                    teacher: r.teacher.unwrap_or_else(|| "-".into()),
                    teacher_id: r.teacher_id,
                    pamong_id: r.pamong_id,
                    category: r.category.filter(|c| !c.is_empty()).unwrap_or_else(|| "-".into()),
                }
            })
            .collect()
    };

    Ok(SessionsData {
        role: user.role.clone(),
        all_scope,
        upcoming: to_items(upcoming_rows?),
        past: to_items(past_rows?),
    })
}

// ── Detail sesi (staf): absensi + chat + rekaman ─────────────────────────────

fn att_display(status: Option<&str>) -> (&'static str, &'static str) {
    match status {
        Some("present") => ("HADIR", "present"),
        Some("late") => ("TERLAMBAT", "late"),
        Some("absent") => ("ALPA", "absent"),
        Some("permit") => ("IZIN", "permit"),
        Some("sick") => ("SAKIT", "sick"),
        Some("outside_schedule") => ("DI LUAR JADWAL", "late"),
        Some(_) => ("—", "none"),
        None => ("BELUM TERCATAT", "none"),
    }
}

fn size_label(bytes: Option<i64>) -> String {
    match bytes {
        Some(b) if b >= 1_048_576 => format!("{:.1} MB", b as f64 / 1_048_576.0),
        Some(b) if b >= 1024 => format!("{} KB", b / 1024),
        Some(b) => format!("{b} B"),
        None => String::new(),
    }
}

/// Detail satu sesi untuk STAF (guru/pamong/dewan/admin): info + daftar absensi
/// anggota kelas + transkrip chat + rekaman (bila ada).
pub async fn detail_for(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
) -> Result<crate::models::SessionDetailData> {
    if !crate::models::role_satisfies(&user.role, &["admin", "supervisor", "dewan_guru", "teacher"]) {
        bail_user!("forbidden");
    }
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        bail_user!("Sesi tidak ditemukan.");
    };

    let (att_rows, chat_rows, teachers, pamongs, books) = tokio::join!(
        repo::session_attendance(pool, session_id, d.class_id),
        repo::session_chats(pool, session_id, 200),
        repo::teacher_options(pool),
        repo::pamong_options(pool),
        repo::books_in_curriculum(pool, d.class_id),
    );

    let wib_tz = crate::service::fmt::wib();
    let mut hadir = 0i64;
    let attendance: Vec<crate::models::SessionAttRow> = att_rows?
        .into_iter()
        .map(|r| {
            let (label, kind) = att_display(r.status.as_deref());
            if matches!(kind, "present" | "late") {
                hadir += 1;
            }
            crate::models::SessionAttRow {
                user_id: r.user_id,
                name: r.full_name,
                nis: r.nis.unwrap_or_else(|| "-".into()),
                status_label: label.into(),
                status_kind: kind.into(),
                time_label: r
                    .scanned_at
                    .map(|t| format!("{} WIB", t.with_timezone(&wib_tz).format("%H:%M")))
                    .unwrap_or_default(),
                att_id: r.att_id,
            }
        })
        .collect();
    let total = attendance.len() as i64;

    let chats = chat_rows?
        .into_iter()
        .map(|(name, message, at)| crate::models::SessionChatItem {
            name,
            message,
            time_label: format!("{}", at.with_timezone(&wib_tz).format("%d %b %H:%M")),
        })
        .collect();

    let (status_label, status_kind) = status_display(&d.status);
    let recording_label = if d.recording_path.is_some() {
        let s = size_label(d.recording_size);
        if s.is_empty() { "Rekaman tersedia".into() } else { format!("Rekaman tersedia · {s}") }
    } else {
        "Belum ada rekaman untuk sesi ini.".into()
    };

    Ok(crate::models::SessionDetailData {
        id: d.id,
        class_id: d.class_id,
        title: d.title.unwrap_or_else(|| d.class_name.clone()),
        class_name: d.class_name,
        date_label: fmt_date(d.session_date),
        time_label: d
            .start_time
            .map(|t| format!("{} WIB", t.format("%H:%M")))
            .unwrap_or_else(|| "-".into()),
        status_label: status_label.into(),
        status_kind: status_kind.into(),
        // Sesi tanpa guru pengisi → WALI KELAS yang bertugas (aturan sama
        // dengan penjagaan koreksi/verifikasi, yang sejak awal memakai
        // COALESCE(teacher_id, wali_kelas_id)). Ditandai agar jelas ini
        // penugasan turunan, bukan pilihan pamong.
        teacher: d
            .teacher
            .clone()
            .or_else(|| d.wali_name.clone().map(|w| format!("{w} (wali kelas)")))
            .unwrap_or_else(|| "-".into()),
        hadir,
        total,
        attendance,
        chats,
        recording_url: d.recording_path,
        recording_label,
        // Fallback ke wali/pamong KELAS bila sesi belum menetapkan petugasnya —
        // cocok dengan COALESCE di query koreksi.
        can_correct: [d.teacher_id, d.pamong_id, d.class_wali_id, d.class_pamong_id]
            .into_iter()
            .flatten()
            .any(|id| id == user.id),
        teacher_id: d.teacher_id,
        teacher_options: teachers?
            .into_iter()
            .map(|(id, name)| crate::models::TeacherOption { id, name })
            .collect(),
        pamong_id: d.pamong_id,
        pamong_options: pamongs?
            .into_iter()
            .map(|(id, name)| crate::models::TeacherOption { id, name })
            .collect(),
        category: d.category.clone().filter(|c| !c.is_empty()).unwrap_or_else(|| "-".into()),
        class_category: d.class_category.clone(),
        start_blocked_reason: start_window_reason(d.session_date, d.start_time, d.end_time),
        book_id: d.book_id,
        book_title: d.book_title,
        book_pages_label: crate::service::books::format_page_ranges(&d.book_pages),
        // HANYA materi yang ada di kurikulum kelas sesi ini — sesi mengajarkan
        // apa yang direncanakan kelasnya, bukan kitab sembarangan.
        book_options: books?
            .into_iter()
            .map(|b| crate::models::BookItem { id: b.id, title: b.title, category: b.category, total_pages: b.total_pages, surahs: Vec::new() })
            .collect(),
        target_book_id: d.target_book_id,
        target_book_title: d.target_book_title,
        target_pages_label: crate::service::books::format_page_ranges(&d.target_pages),
        actual_detail: d.actual_detail,
    })
}

/// Tandai santri HADIR manual pada sesi (staf). Masuk antrean verifikasi normal.
pub async fn mark_present(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
    student_id: i64,
) -> Result<()> {
    if !crate::models::role_satisfies(&user.role, &["admin", "supervisor", "dewan_guru", "teacher"]) {
        bail_user!("forbidden");
    }
    if !repo::mark_manual_present(pool, student_id, session_id).await? {
        bail_user!("Sudah tercatat sebelumnya.");
    }
    tracing::info!(by = user.id, student_id, session_id, "absensi manual ditandai staf");
    Ok(())
}

/// Tandai BANYAK santri sekaligus (hadir/alpa) pada sesi (staf). Return jumlah
/// baru tercatat.
pub async fn mark_bulk(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
    student_ids: Vec<i64>,
    status: &str,
) -> Result<i64> {
    if !crate::models::role_satisfies(&user.role, &["admin", "supervisor", "dewan_guru", "teacher"]) {
        bail_user!("forbidden");
    }
    if !matches!(status, "present" | "absent") {
        bail_user!("Status tidak valid.");
    }
    let ids: Vec<i64> = student_ids.into_iter().filter(|&x| x > 0).collect();
    if ids.is_empty() {
        bail_user!("Pilih minimal satu santri.");
    }
    let n = repo::mark_attendance_bulk(pool, session_id, &ids, status).await?;
    tracing::info!(by = user.id, session_id, status, n, "absensi massal ditandai staf");
    Ok(n)
}

// ── Ruang sesi LIVE (/sesi/:id/live): chat + mulai/akhiri ────────────────────
// CATATAN: suara online (WebRTC SFU) BELUM ada — halaman ini menyiapkan ruang
// (status live + chat); audio dicolok menyusul (butuh subsistem WS+SFU).

fn is_staff(role: &str) -> bool {
    matches!(role, "admin" | "supervisor" | "dewan_guru" | "teacher")
}

/// Akses ruang live: staf ATAU santri peserta kelas sesi tsb.
async fn guard_live_access(
    pool: &Pool,
    user: &SessionUser,
    class_id: i64,
) -> Result<()> {
    if is_staff(&user.role) || repo::is_class_participant(pool, class_id, user.id).await? {
        Ok(())
    } else {
        bail_user!("forbidden")
    }
}

pub async fn live_for(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
) -> Result<crate::models::SessionLiveData> {
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        bail_user!("Sesi tidak ditemukan.");
    };
    guard_live_access(pool, user, d.class_id).await?;

    let (chats, member_count) = tokio::join!(
        repo::session_chats(pool, session_id, 200),
        repo::class_member_count(pool, d.class_id),
    );
    let member_count = member_count?;

    let wib_tz = crate::service::fmt::wib();
    let chats = chats?
        .into_iter()
        .map(|(name, message, at)| crate::models::SessionChatItem {
            name,
            message,
            time_label: format!("{}", at.with_timezone(&wib_tz).format("%H:%M")),
        })
        .collect();

    let can_record = d.category.as_deref().is_some_and(category_allows_recording);
    let start_blocked_reason = start_window_reason(d.session_date, d.start_time, d.end_time);

    Ok(crate::models::SessionLiveData {
        id: d.id,
        title: d.title.unwrap_or_else(|| d.class_name.clone()),
        class_name: d.class_name,
        teacher: d.teacher.unwrap_or_else(|| "Belum ditentukan".into()),
        status_kind: d.status.clone(),
        can_manage: is_staff(&user.role),
        chats,
        member_count,
        recording_url: (d.status == "finished").then_some(d.recording_path).flatten(),
        can_record,
        start_blocked_reason,
    })
}

/// Kirim pesan chat sesi (staf & santri peserta). Panjang 1..500.
pub async fn post_chat(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
    message: &str,
) -> Result<()> {
    let msg = message.trim();
    if msg.is_empty() || msg.chars().count() > 500 {
        bail_user!("Pesan 1–500 karakter.");
    }
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        bail_user!("Sesi tidak ditemukan.");
    };
    guard_live_access(pool, user, d.class_id).await?;
    // Chat HANYA saat sesi berlangsung. Sebelum dimulai tak ada yang menyimak;
    // setelah berakhir, pesan yang masuk tak akan pernah terbaca guru dan
    // membuat riwayat sesi seolah masih hidup. Ditegakkan di server, bukan cuma
    // menyembunyikan kotak ketiknya di UI.
    match d.status.as_str() {
        "ongoing" => {}
        "cancelled" => bail_user!("Sesi dibatalkan (libur)."),
        "finished" => bail_user!("Sesi sudah berakhir — chat ditutup."),
        _ => bail_user!("Sesi belum dimulai — chat dibuka saat sesi berlangsung."),
    }
    repo::insert_session_chat(pool, session_id, user.id, msg).await
}

/// Mulai (ongoing) / akhiri (finished) sesi — staf saja. MULAI (start=true)
/// dibatasi jendela ±10 menit dari jadwal (start_window_reason); AKHIRI bebas
/// kapan saja (staf harus selalu bisa menutup sesi yang lupa diakhiri).
pub async fn set_live(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
    start: bool,
) -> Result<()> {
    if !is_staff(&user.role) {
        bail_user!("forbidden");
    }
    if start {
        let Some(d) = repo::session_detail(pool, session_id).await? else {
            bail_user!("Sesi tidak ditemukan.");
        };
        if let Some(reason) = start_window_reason(d.session_date, d.start_time, d.end_time) {
            bail_user!(reason);
        }
    }
    let status = if start { "ongoing" } else { "finished" };
    if !repo::set_session_status(pool, session_id, status).await? {
        bail_user!("Sesi tidak ditemukan.");
    }
    tracing::info!(by = user.id, session_id, status, "status sesi live diubah");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::start_window_reason;
    use chrono::NaiveDate;

    fn d() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 7, 20).unwrap()
    }

    #[test]
    fn tanpa_jadwal_tak_dibatasi() {
        assert!(start_window_reason(d(), None, None).is_none());
    }

    #[test]
    fn jendela_dihitung_dari_tanggal_sesi_bukan_hari_ini() {
        // Sesi tanggal lampau/mendatang (bukan hari ini) → selalu di luar
        // jendela ±10 menit relatif "sekarang" nyata → harus terblokir.
        let far_past = NaiveDate::from_ymd_opt(2020, 1, 1).unwrap();
        let start = chrono::NaiveTime::from_hms_opt(4, 30, 0);
        let end = chrono::NaiveTime::from_hms_opt(5, 0, 0);
        assert!(start_window_reason(far_past, start, end).is_some());
    }
}
