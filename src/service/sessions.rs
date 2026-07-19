//! service/sessions.rs — Daftar sesi kelas per-peran.
//! Santri → hanya sesi kelas yang diikutinya; admin/pamong/dewan guru → SEMUA
//! (nantinya bisa mengelola/update sesi dari halaman yang sama).

use anyhow::Result;
use deadpool_postgres::Pool;

use chrono::{Duration, Utc};

use super::fmt::{fmt_date, wib};
use crate::models::{SessionItem, SessionsData, SessionUser};
use crate::repository as repo;

fn status_display(status: &str) -> (&'static str, &'static str) {
    match status {
        "ongoing" => ("Berlangsung", "ongoing"),
        "finished" => ("Selesai", "finished"),
        "cancelled" => ("Dibatalkan", "cancelled"),
        _ => ("Terjadwal", "scheduled"),
    }
}

pub async fn list_for(pool: &Pool, user: &SessionUser) -> Result<SessionsData> {
    // Hanya sesi 1 MINGGU TERAKHIR yang sudah lewat (s/d hari ini WIB).
    let until = Utc::now().with_timezone(&wib()).date_naive();
    let since = until - Duration::days(7);
    let all_scope = matches!(user.role.as_str(), "admin" | "supervisor" | "dewan_guru");
    let rows = if all_scope {
        repo::all_sessions(pool, since, until, 100).await?
    } else if user.role == "teacher" {
        repo::sessions_for_teacher(pool, user.id, since, until, 100).await?
    } else {
        repo::sessions_for_student(pool, user.id, since, until, 100).await?
    };

    let items = rows
        .into_iter()
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
            }
        })
        .collect();

    Ok(SessionsData {
        role: user.role.clone(),
        all_scope,
        items,
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
    if !matches!(user.role.as_str(), "admin" | "supervisor" | "dewan_guru" | "teacher") {
        anyhow::bail!("forbidden");
    }
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        anyhow::bail!("Sesi tidak ditemukan.");
    };

    let (att_rows, chat_rows) = tokio::join!(
        repo::session_attendance(pool, session_id, d.class_id),
        repo::session_chats(pool, session_id, 200),
    );

    let wib_tz = crate::service::fmt::wib();
    let mut hadir = 0i64;
    let attendance: Vec<crate::models::SessionAttRow> = att_rows?
        .into_iter()
        .map(|(user_id, name, nis, status, at)| {
            let (label, kind) = att_display(status.as_deref());
            if matches!(kind, "present" | "late") {
                hadir += 1;
            }
            crate::models::SessionAttRow {
                user_id,
                name,
                nis: nis.unwrap_or_else(|| "-".into()),
                status_label: label.into(),
                status_kind: kind.into(),
                time_label: at
                    .map(|t| format!("{} WIB", t.with_timezone(&wib_tz).format("%H:%M")))
                    .unwrap_or_default(),
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
        title: d.title.unwrap_or_else(|| d.class_name.clone()),
        class_name: d.class_name,
        date_label: fmt_date(d.session_date),
        time_label: d
            .start_time
            .map(|t| format!("{} WIB", t.format("%H:%M")))
            .unwrap_or_else(|| "-".into()),
        status_label: status_label.into(),
        status_kind: status_kind.into(),
        teacher: d.teacher.unwrap_or_else(|| "-".into()),
        hadir,
        total,
        attendance,
        chats,
        recording_url: d.recording_path,
        recording_label,
    })
}

/// Tandai santri HADIR manual pada sesi (staf). Masuk antrean verifikasi normal.
pub async fn mark_present(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
    student_id: i64,
) -> Result<()> {
    if !matches!(user.role.as_str(), "admin" | "supervisor" | "dewan_guru" | "teacher") {
        anyhow::bail!("forbidden");
    }
    if !repo::mark_manual_present(pool, student_id, session_id).await? {
        anyhow::bail!("Sudah tercatat sebelumnya.");
    }
    tracing::info!(by = user.id, student_id, session_id, "absensi manual ditandai staf");
    Ok(())
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
        anyhow::bail!("forbidden")
    }
}

pub async fn live_for(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
) -> Result<crate::models::SessionLiveData> {
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        anyhow::bail!("Sesi tidak ditemukan.");
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

    Ok(crate::models::SessionLiveData {
        id: d.id,
        title: d.title.unwrap_or_else(|| d.class_name.clone()),
        class_name: d.class_name,
        teacher: d.teacher.unwrap_or_else(|| "Belum ditentukan".into()),
        status_kind: d.status.clone(),
        can_manage: is_staff(&user.role),
        chats,
        member_count,
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
        anyhow::bail!("Pesan 1–500 karakter.");
    }
    let Some(d) = repo::session_detail(pool, session_id).await? else {
        anyhow::bail!("Sesi tidak ditemukan.");
    };
    guard_live_access(pool, user, d.class_id).await?;
    if d.status == "cancelled" {
        anyhow::bail!("Sesi dibatalkan (libur).");
    }
    repo::insert_session_chat(pool, session_id, user.id, msg).await
}

/// Mulai (ongoing) / akhiri (finished) sesi — staf saja.
pub async fn set_live(
    pool: &Pool,
    user: &SessionUser,
    session_id: i64,
    start: bool,
) -> Result<()> {
    if !is_staff(&user.role) {
        anyhow::bail!("forbidden");
    }
    let status = if start { "ongoing" } else { "finished" };
    if !repo::set_session_status(pool, session_id, status).await? {
        anyhow::bail!("Sesi tidak ditemukan.");
    }
    tracing::info!(by = user.id, session_id, status, "status sesi live diubah");
    Ok(())
}
