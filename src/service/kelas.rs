//! service/kelas.rs — Manajemen kelas (admin/guru/dewan guru/pamong): daftar
//! kelas, detail (anggota + jadwal + sesi), buat/ubah kelas, kategori fleksibel,
//! jadwal (buat/ubah/hapus + generate sesi bulanan), tambah/keluarkan santri,
//! serta payload halaman Students (daftar santri + antrean verifikasi per-peran).

use anyhow::{bail, Result};
use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Utc};
use deadpool_postgres::Pool;

use super::fmt::{fmt_date, fmt_when, wib};
use crate::models::{
    CurriculumItem, KelasData, KelasDetail, KelasItem, MemberItem, PendingAtt, ScheduleItem,
    ScheduleOption, SessionItem, SessionUser, StudentClassTag, StudentRowItem, StudentSearchItem,
    StudentsData, TeacherOption,
};
use crate::repository as repo;

/// Tanggal-tanggal yang cocok pola recurrence dalam rentang [from, to] inklusif.
fn dates_in_range(rec: &str, start_date: NaiveDate, from: NaiveDate, to: NaiveDate) -> Vec<NaiveDate> {
    let mut dates = Vec::new();
    let mut d = from;
    while d <= to {
        if d >= start_date {
            let hit = match rec {
                "daily" => true,
                "weekly" => d.weekday() == start_date.weekday(),
                "monthly" => d.day() == start_date.day(),
                "once" => d == start_date,
                // 'custom' = daftar tanggal manual → dimaterialisasi LANGSUNG dari
                // custom_dates saat buat/ubah (bukan lewat pola), jadi tak cocok
                // apa pun di sini.
                _ => false,
            };
            if hit {
                dates.push(d);
            }
        }
        match d.succ_opt() {
            Some(n) => d = n,
            None => break,
        }
    }
    dates
}

/// Auto-materialisasi sesi MENDATANG (hari ini s/d 7 hari ke depan) dari semua
/// jadwal aktif kelas — idempotent (insert_sessions melewati duplikat). Dipanggil
/// saat BUAT jadwal (bukan tiap buka halaman) agar sesi minggu ini siap diisi.
async fn ensure_upcoming_sessions(pool: &Pool, class_id: i64) -> Result<()> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let horizon = today + Duration::days(7);
    for (sid, title, rec, start_date, end_date) in repo::active_schedules_of(pool, class_id).await? {
        let from = today.max(start_date);
        // JANGAN materialisasi melewati end_date jadwal (BUG lama: selalu +7 hari
        // → sesi di luar rentang dibuat ULANG tepat setelah update_schedule).
        let to = end_date.map_or(horizon, |ed| horizon.min(ed));
        let dates = dates_in_range(&rec, start_date, from, to);
        let title = if title.trim().is_empty() {
            "Sesi Kelas".to_string()
        } else {
            title
        };
        // Best-effort: kegagalan satu jadwal tak menggagalkan pemuatan detail.
        let _ = repo::insert_sessions(pool, class_id, sid, &title, &dates).await;
    }
    Ok(())
}

/// Materialisasi sesi mendatang untuk SEMUA kelas (dipakai task background
/// main.rs, di luar jalur request). Idempotent. Return jumlah sesi baru.
pub async fn ensure_upcoming_all(pool: &Pool) -> Result<i64> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let horizon = today + Duration::days(7);
    let mut total = 0i64;
    for (class_id, sid, title, rec, start_date, end_date) in repo::active_schedules_all(pool).await? {
        let from = today.max(start_date);
        let to = end_date.map_or(horizon, |ed| horizon.min(ed));
        let dates = dates_in_range(&rec, start_date, from, to);
        let title = if title.trim().is_empty() { "Sesi Kelas".to_string() } else { title };
        total += repo::insert_sessions(pool, class_id, sid, &title, &dates).await.unwrap_or(0);
    }
    Ok(total)
}

/// `end_date` jadwal (kalau diisi) WAJIB ≥ BESOK. Hari ini tak boleh jadi akhir:
/// mungkin sudah ada sesi hari ini yang berjalan. Untuk membatalkan sesi hari
/// ini, tandai sesi sebagai LIBUR — bukan memundurkan akhir jadwal.
fn validate_end_date(ed: Option<NaiveDate>, today: NaiveDate) -> Result<()> {
    if let Some(end) = ed {
        if end <= today {
            bail!(
                "Tanggal berakhir jadwal minimal BESOK. Hari ini mungkin ada sesi \
                 yang sudah berjalan — untuk membatalkannya, tandai sesi sebagai LIBUR."
            );
        }
    }
    Ok(())
}

/// Parse input poin (kosong = None → pakai default). SEMUA poin kini MAGNITUDO
/// POSITIF & konsisten (migrasi 21): present ditambah, late/absent dikurangi —
/// arah operasi ditentukan di models::attendance_delta, bukan tandanya. Nilai
/// minus ditolak (menghilangkan kebingungan lama saat late_points bertanda).
fn parse_point_magnitude(s: &str, field: &str) -> Result<Option<i16>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    let n: i16 = s
        .parse()
        .map_err(|_| anyhow::anyhow!("Poin {field} harus berupa angka positif (mis. 10)."))?;
    if !(0..=100).contains(&n) {
        bail!("Poin {field} harus di antara 0 sampai 100 (tanpa minus).");
    }
    Ok(Some(n))
}

/// Parse daftar tanggal manual "2026-07-24,2026-08-01" → Vec<NaiveDate> unik &
/// terurut. Untuk recurrence 'custom'. Toleran spasi & pemisah baris/koma.
fn parse_custom_dates(s: &str) -> Result<Vec<NaiveDate>> {
    let mut out = Vec::new();
    for part in s.split([',', '\n', ' ']) {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let d = NaiveDate::parse_from_str(p, "%Y-%m-%d")
            .map_err(|_| anyhow::anyhow!("Tanggal tidak valid: \"{p}\" (format YYYY-MM-DD)."))?;
        out.push(d);
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// JSONB array ISO dari Vec<NaiveDate> (disimpan di class_schedules.custom_dates).
fn custom_dates_json(dates: &[NaiveDate]) -> serde_json::Value {
    serde_json::Value::Array(
        dates.iter().map(|d| serde_json::Value::String(d.format("%Y-%m-%d").to_string())).collect(),
    )
}

fn recurrence_label(t: &str) -> &'static str {
    match t {
        "daily" => "Harian",
        "weekly" => "Mingguan",
        "monthly" => "Bulanan",
        "custom" => "Tanggal tertentu",
        _ => "Sekali",
    }
}

fn curriculum_status_label(status: &str) -> &'static str {
    match status {
        "completed" => "Selesai",
        "upcoming" => "Akan Datang",
        _ => "Berjalan",
    }
}

fn session_status(status: &str) -> (&'static str, &'static str) {
    match status {
        "ongoing" => ("Berlangsung", "ongoing"),
        "finished" => ("Selesai", "finished"),
        "cancelled" => ("Dibatalkan", "cancelled"),
        _ => ("Terjadwal", "scheduled"),
    }
}

/// Angkatan santri = 4 digit awal NIS bila berupa tahun (mis. 2023001 → "2023").
fn angkatan_from_nis(nis: &str) -> String {
    let head: String = nis.chars().take(4).collect();
    match head.parse::<i32>() {
        Ok(y) if (1900..=2100).contains(&y) => head,
        _ => String::new(),
    }
}

fn initial_of(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .filter_map(|w| w.chars().next())
        .collect::<String>()
        .to_uppercase()
}

/// Perkiraan sesi per minggu dari pola recurrence (untuk statistik jadwal).
fn weekly_of(rec: &str) -> i64 {
    match rec {
        "daily" => 7,
        "weekly" => 1,
        _ => 0,
    }
}

/// Daftar kelas + statistik untuk halaman /kelas.
pub async fn kelas_list(pool: &Pool, role: &str) -> Result<KelasData> {
    let (totals, classes) = tokio::join!(repo::class_totals(pool), repo::list_classes(pool));
    let (total_kelas, total_santri) = totals?;
    let items = classes?
        .into_iter()
        .map(|c| KelasItem {
            id: c.id,
            name: c.name,
            description: c.description,
            category: c.category.unwrap_or_default(),
            golongan: c.golongan.unwrap_or_default(),
            teacher: c.teacher,
            member_count: c.member_count,
            schedule_count: c.schedule_count,
        })
        .collect();
    Ok(KelasData { role: role.to_string(), total_kelas, total_santri, items })
}

/// Detail satu kelas (anggota, jadwal, sesi, kategori, opsi form, statistik).
pub async fn kelas_detail(pool: &Pool, role: &str, class_id: i64) -> Result<KelasDetail> {
    let Some((name, description, category, golongan)) = repo::class_info(pool, class_id).await?
    else {
        bail!("Kelas tidak ditemukan.");
    };

    // CATATAN PERF: materialisasi sesi (menulis) TIDAK lagi di sini — dulu tiap
    // GET detail menulis sesi (serial per-jadwal) → lambat, apalagi DB remote.
    // Kini dilakukan: (1) saat BUAT jadwal, (2) task background 600s (semua
    // kelas) di main.rs. Halaman detail = murni baca (5 query paralel).
    // Sesi yang DITAMPILKAN hanya MULAI hari ini ke depan (yang lewat dibuang).
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let (members, scheds, sessions, teachers, cats, golongans, curriculum, books, rooms) = tokio::join!(
        repo::class_members(pool, class_id),
        repo::class_schedules(pool, class_id),
        repo::sessions_of_class(pool, class_id, today, 50),
        repo::teacher_options(pool),
        repo::distinct_categories(pool),
        repo::distinct_golongan(pool),
        repo::class_curriculum(pool, class_id),
        repo::list_books(pool),
        repo::device_options(pool),
    );

    let members = members?
        .into_iter()
        .map(|(id, name, nis)| {
            let nis = nis.unwrap_or_default();
            MemberItem {
                angkatan: angkatan_from_nis(&nis),
                nis: if nis.is_empty() { "-".into() } else { nis },
                id,
                name,
            }
        })
        .collect();

    let scheds = scheds?;
    let weekly_sessions: i64 = scheds.iter().map(|s| weekly_of(&s.recurrence_type)).sum();
    let durations: Vec<i64> = scheds
        .iter()
        .map(|s| (s.end_time - s.start_time).num_minutes().max(0))
        .collect();
    let avg_duration_min = if durations.is_empty() {
        0
    } else {
        durations.iter().sum::<i64>() / durations.len() as i64
    };

    let schedule_options = scheds
        .iter()
        .map(|s| ScheduleOption {
            id: s.id,
            label: format!(
                "{} ({}–{})",
                if s.title.is_empty() { "Jadwal" } else { &s.title },
                s.start_time.format("%H:%M"),
                s.end_time.format("%H:%M")
            ),
        })
        .collect();
    let schedules = scheds
        .into_iter()
        .map(|s| ScheduleItem {
            duration_min: (s.end_time - s.start_time).num_minutes().max(0),
            title: if s.title.is_empty() {
                "Jadwal Kelas".into()
            } else {
                s.title
            },
            time_label: format!(
                "{} – {} WIB",
                s.start_time.format("%H:%M"),
                s.end_time.format("%H:%M")
            ),
            recurrence_label: recurrence_label(&s.recurrence_type).into(),
            date_label: format!("Mulai {}", fmt_date(s.start_date)),
            start_hm: s.start_time.format("%H:%M").to_string(),
            end_hm: s.end_time.format("%H:%M").to_string(),
            limit_hm: s.limit_time.format("%H:%M").to_string(),
            start_date: s.start_date.format("%Y-%m-%d").to_string(),
            end_date: s
                .end_date
                .map(|d| d.format("%Y-%m-%d").to_string())
                .unwrap_or_default(),
            recurrence: s.recurrence_type,
            id: s.id,
            category: s.category.clone().unwrap_or_default(),
            present_points: s.present_points.map(|n| n.to_string()).unwrap_or_default(),
            late_points: s.late_points.map(|n| n.to_string()).unwrap_or_default(),
            absent_points: s.absent_points.map(|n| n.to_string()).unwrap_or_default(),
            room_id: s.room_id.unwrap_or(0),
            room_label: s.room_name.clone().unwrap_or_default(),
            custom_dates: s.custom_dates.join(","),
        })
        .collect();

    // Urut MENAIK per tanggal → sesi terdekat/akan datang tampil natural.
    let mut sess_rows = sessions?;
    sess_rows.sort_by_key(|r| r.session_date);
    let sessions = sess_rows
        .into_iter()
        .map(|r| {
            let (status_label, status_kind) = session_status(&r.status);
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
                category: r.category.filter(|c| !c.is_empty()).unwrap_or_else(|| "-".into()),
            }
        })
        .collect();

    let teacher_options = teachers?
        .into_iter()
        .map(|(id, name)| TeacherOption { id, name })
        .collect();

    let curriculum = curriculum?
        .into_iter()
        .map(|c| CurriculumItem {
            id: c.id,
            title: c.title,
            description: c.description.unwrap_or_default(),
            scope_start: c.scope_start.unwrap_or_default(),
            scope_end: c.scope_end.unwrap_or_default(),
            progress_pct: c.progress_pct,
            order_index: c.order_index,
            status_label: curriculum_status_label(&c.status).into(),
            status: c.status,
            book_id: c.book_id.unwrap_or(0),
            book_title: c.book_title.unwrap_or_default(),
        })
        .collect();

    let book_options = books?
        .into_iter()
        .map(|b| crate::models::BookItem { id: b.id, title: b.title, total_pages: b.total_pages })
        .collect();
    let room_options = rooms?
        .into_iter()
        .map(|(id, name)| crate::models::RoomOption { id, name })
        .collect();

    Ok(KelasDetail {
        role: role.to_string(),
        id: class_id,
        name,
        description,
        category: category.unwrap_or_default(),
        category_options: cats?,
        golongan: golongan.unwrap_or_default(),
        golongan_options: golongans?,
        members,
        schedules,
        schedule_options,
        teacher_options,
        room_options,
        book_options,
        sessions,
        weekly_sessions,
        avg_duration_min,
        curriculum,
    })
}

fn norm_category(category: &str) -> Option<String> {
    let c = category.trim();
    if c.is_empty() {
        None
    } else {
        Some(c.to_string())
    }
}

pub async fn create_class(
    pool: &Pool,
    name: &str,
    category: &str,
    golongan: &str,
    description: &str,
) -> Result<i64> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Nama kelas wajib diisi.");
    }
    repo::create_class(
        pool,
        name,
        norm_category(category).as_deref(),
        norm_category(golongan).as_deref(),
        description.trim(),
    )
    .await
}

pub async fn update_class(
    pool: &Pool,
    class_id: i64,
    name: &str,
    category: &str,
    golongan: &str,
) -> Result<()> {
    let name = name.trim();
    if name.is_empty() {
        bail!("Nama kelas wajib diisi.");
    }
    if !repo::update_class(
        pool,
        class_id,
        name,
        norm_category(category).as_deref(),
        norm_category(golongan).as_deref(),
    )
    .await?
    {
        bail!("Kelas tidak ditemukan.");
    }
    Ok(())
}

pub async fn categories(pool: &Pool) -> Result<Vec<String>> {
    repo::distinct_categories(pool).await
}

fn parse_time(s: &str, field: &str) -> Result<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M")
        .map_err(|_| anyhow::anyhow!("Format {field} tidak valid (HH:MM)."))
}

fn parse_date(s: &str, field: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
        .map_err(|_| anyhow::anyhow!("Tanggal {field} tidak valid."))
}

/// Validasi & normalisasi field jadwal (dipakai create & update).
fn parse_schedule_fields(
    start_time: &str,
    end_time: &str,
    limit_time: &str,
    recurrence: &str,
    start_date: &str,
    end_date: &str,
) -> Result<(NaiveTime, NaiveTime, NaiveTime, &'static str, NaiveDate, Option<NaiveDate>)> {
    let st = parse_time(start_time, "jam mulai")?;
    let et = parse_time(end_time, "jam selesai")?;
    if et <= st {
        bail!("Jam selesai harus setelah jam mulai.");
    }
    let lt = if limit_time.trim().is_empty() {
        st
    } else {
        parse_time(limit_time, "batas terlambat")?
    };
    let sd = parse_date(start_date, "mulai")?;
    let ed = if end_date.trim().is_empty() {
        None
    } else {
        Some(parse_date(end_date, "selesai")?)
    };
    let rec = match recurrence {
        "once" => "once",
        "weekly" => "weekly",
        "monthly" => "monthly",
        "custom" => "custom",
        _ => "daily",
    };
    Ok((st, et, lt, rec, sd, ed))
}

#[allow(clippy::too_many_arguments)]
pub async fn create_schedule(
    pool: &Pool,
    class_id: i64,
    title: &str,
    start_time: &str,
    end_time: &str,
    limit_time: &str,
    recurrence: &str,
    start_date: &str,
    end_date: &str,
    category: &str,
    present_points: &str,
    late_points: &str,
    absent_points: &str,
    room_id: i64,
    custom_dates: &str,
) -> Result<i64> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    // Recurrence 'custom' = tanggal manual (loncat-loncat): start/end jadwal
    // diturunkan dari min/max tanggal, bukan dari form.
    let custom = if recurrence == "custom" { parse_custom_dates(custom_dates)? } else { Vec::new() };
    if recurrence == "custom" && custom.is_empty() {
        bail!("Pilih minimal satu tanggal untuk jadwal tanggal-tertentu.");
    }
    let (sd_str, ed_str) = if recurrence == "custom" {
        (
            custom.first().unwrap().format("%Y-%m-%d").to_string(),
            custom.last().unwrap().format("%Y-%m-%d").to_string(),
        )
    } else {
        (start_date.to_string(), end_date.to_string())
    };
    let (st, et, lt, rec, sd, ed) =
        parse_schedule_fields(start_time, end_time, limit_time, recurrence, &sd_str, &ed_str)?;
    if rec != "custom" {
        validate_end_date(ed, today)?;
    }
    let cat = category.trim();
    let cat = (!cat.is_empty()).then_some(cat);
    let pp = parse_point_magnitude(present_points, "tepat waktu")?;
    let lp = parse_point_magnitude(late_points, "telat")?;
    let ap = parse_point_magnitude(absent_points, "alpa")?;
    let room = (room_id > 0).then_some(room_id);
    let cd_json = custom_dates_json(&custom);
    let id = repo::create_schedule(
        pool, class_id, title.trim(), st, et, lt, rec, sd, ed, cat, pp, lp, ap, room, &cd_json,
    )
    .await?;
    // Materialisasi sesi. 'custom' → langsung SEMUA tanggal ≥ hari ini (tak
    // dibatasi jendela 7 hari, agar tanggal jauh langsung muncul); pola biasa →
    // rolling 7 hari via ensure_upcoming_sessions.
    if rec == "custom" {
        let future: Vec<NaiveDate> = custom.into_iter().filter(|d| *d >= today).collect();
        let t = if title.trim().is_empty() { "Sesi Kelas".to_string() } else { title.trim().to_string() };
        let _ = repo::insert_sessions(pool, class_id, id, &t, &future).await;
    } else {
        let _ = ensure_upcoming_sessions(pool, class_id).await;
    }
    Ok(id)
}

#[allow(clippy::too_many_arguments)]
pub async fn update_schedule(
    pool: &Pool,
    schedule_id: i64,
    title: &str,
    start_time: &str,
    end_time: &str,
    limit_time: &str,
    recurrence: &str,
    start_date: &str,
    end_date: &str,
    category: &str,
    present_points: &str,
    late_points: &str,
    absent_points: &str,
    room_id: i64,
    custom_dates: &str,
) -> Result<()> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let custom = if recurrence == "custom" { parse_custom_dates(custom_dates)? } else { Vec::new() };
    if recurrence == "custom" && custom.is_empty() {
        bail!("Pilih minimal satu tanggal untuk jadwal tanggal-tertentu.");
    }
    let (sd_str, ed_str) = if recurrence == "custom" {
        (
            custom.first().unwrap().format("%Y-%m-%d").to_string(),
            custom.last().unwrap().format("%Y-%m-%d").to_string(),
        )
    } else {
        (start_date.to_string(), end_date.to_string())
    };
    let (st, et, lt, rec, sd, ed) =
        parse_schedule_fields(start_time, end_time, limit_time, recurrence, &sd_str, &ed_str)?;
    if rec != "custom" {
        validate_end_date(ed, today)?;
    }
    let cat = category.trim();
    let cat = (!cat.is_empty()).then_some(cat);
    let pp = parse_point_magnitude(present_points, "tepat waktu")?;
    let lp = parse_point_magnitude(late_points, "telat")?;
    let ap = parse_point_magnitude(absent_points, "alpa")?;
    let room = (room_id > 0).then_some(room_id);
    let cd_json = custom_dates_json(&custom);
    if !repo::update_schedule(
        pool, schedule_id, title.trim(), st, et, lt, rec, sd, ed, cat, pp, lp, ap, room, &cd_json,
    )
    .await?
    {
        bail!("Jadwal tidak ditemukan.");
    }

    // Sinkronkan SESI MENDATANG: hapus sesi mendatang yang kini DI LUAR
    // rentang/pola/daftar-tanggal baru & belum dipakai (tanpa absensi/chat),
    // pertahankan yang masih valid, lalu materialisasi ulang. Untuk 'custom',
    // himpunan valid = semua tanggal manual ≥ hari ini.
    let valid: Vec<NaiveDate> = if rec == "custom" {
        custom.iter().cloned().filter(|d| *d >= today).collect()
    } else {
        let upper = ed.unwrap_or(today + Duration::days(400));
        dates_in_range(&rec, sd, today.max(sd), upper)
    };
    match repo::delete_future_sessions_not_in(pool, schedule_id, today, &valid).await {
        Ok(n) => tracing::info!(schedule_id, valid = valid.len(), "sync sesi: {n} sesi mendatang dihapus (di luar rentang/pola)"),
        Err(e) => tracing::warn!(schedule_id, "sync sesi GAGAL: {e}"),
    }
    if let Some((class_id, title_db, ..)) = repo::schedule_info(pool, schedule_id).await? {
        if rec == "custom" {
            let t = if title_db.trim().is_empty() { "Sesi Kelas".to_string() } else { title_db };
            let _ = repo::insert_sessions(pool, class_id, schedule_id, &t, &valid).await;
        } else {
            let _ = ensure_upcoming_sessions(pool, class_id).await;
        }
    }
    Ok(())
}

pub async fn delete_schedule(pool: &Pool, schedule_id: i64) -> Result<()> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    if !repo::delete_schedule(pool, schedule_id, today).await? {
        bail!("Jadwal tidak ditemukan.");
    }
    Ok(())
}

/// Generate sesi untuk satu bulan dari sebuah jadwal (materialisasi). Return
/// jumlah sesi baru. Tanggal ditentukan pola recurrence, hanya ≥ start_date.
pub async fn generate_month_sessions(
    pool: &Pool,
    schedule_id: i64,
    year: i32,
    month: u32,
) -> Result<i64> {
    if !(1..=12).contains(&month) {
        bail!("Bulan tidak valid.");
    }
    let Some((class_id, title, rec, start_date)) = repo::schedule_info(pool, schedule_id).await?
    else {
        bail!("Jadwal tidak ditemukan.");
    };
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
        bail!("Bulan/tahun tidak valid.");
    };
    // Akhir bulan = sehari sebelum tanggal 1 bulan berikutnya.
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    let last = NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .unwrap_or(first);

    let dates = dates_in_range(&rec, start_date, first, last);
    let title = if title.trim().is_empty() {
        "Sesi Kelas".to_string()
    } else {
        title
    };
    repo::insert_sessions(pool, class_id, schedule_id, &title, &dates).await
}

/// `book_id` opsional (0/None = tanpa materi buku); `book_pages_text` kotak
/// teks "11-20, 45-50" divalidasi terhadap total_pages buku terpilih (kosong
/// bila book_id tak diisi — reuse parse_page_ranges, service/books.rs).
#[allow(clippy::too_many_arguments)]
pub async fn create_session(
    pool: &Pool,
    class_id: i64,
    schedule_id: Option<i64>,
    teacher_id: Option<i64>,
    title: &str,
    session_date: &str,
    book_id: Option<i64>,
    book_pages_text: &str,
) -> Result<i64> {
    let date = parse_date(session_date, "sesi")?;
    let sched = schedule_id.filter(|v| *v > 0);
    let teacher = teacher_id.filter(|v| *v > 0);
    let book = book_id.filter(|v| *v > 0);
    let pages = book_pages_value(pool, book, book_pages_text).await?;
    repo::create_session(pool, class_id, sched, teacher, title.trim(), date, book, &pages).await
}

/// Validasi rentang halaman terhadap `books.total_pages` bila `book_id`
/// terisi; kosong (`[]`) bila tidak ada buku dipilih.
async fn book_pages_value(
    pool: &Pool,
    book_id: Option<i64>,
    pages_text: &str,
) -> Result<serde_json::Value> {
    match book_id {
        Some(id) => {
            let Some(book) = repo::get_book(pool, id).await? else {
                bail!("Buku tidak ditemukan.");
            };
            super::books::parse_page_ranges(pages_text, book.total_pages)
        }
        None => Ok(serde_json::Value::Array(Vec::new())),
    }
}

/// Ubah materi buku sesi yang SUDAH ada (tab "Kelola" /sesi/:id) — sama pola
/// dgn `set_session_teacher`.
pub async fn set_session_book(
    pool: &Pool,
    session_id: i64,
    book_id: i64,
    book_pages_text: &str,
) -> Result<()> {
    let book = Some(book_id).filter(|v| *v > 0);
    let pages = book_pages_value(pool, book, book_pages_text).await?;
    if !repo::set_session_book(pool, session_id, book, &pages).await? {
        bail!("Sesi tidak ditemukan.");
    }
    Ok(())
}

pub async fn add_member(pool: &Pool, class_id: i64, schedule_id: i64, student_id: i64) -> Result<()> {
    if schedule_id <= 0 {
        bail!("Pilih jadwal untuk menempatkan santri.");
    }
    if !repo::add_member(pool, class_id, schedule_id, student_id).await? {
        bail!("Santri sudah terdaftar pada jadwal ini.");
    }
    Ok(())
}

pub async fn remove_member(pool: &Pool, class_id: i64, student_id: i64) -> Result<()> {
    if !repo::remove_member(pool, class_id, student_id).await? {
        bail!("Santri tidak ada di kelas ini.");
    }
    Ok(())
}

/// Cari santri untuk ditambahkan ke kelas. Query pendek/kosong → daftar DEFAULT
/// (beberapa santri) supaya form tak kosong sebelum mengetik.
pub async fn search_students(pool: &Pool, q: &str) -> Result<Vec<StudentSearchItem>> {
    if q.trim().chars().count() < 2 {
        return Ok(repo::some_students(pool, 15)
            .await?
            .into_iter()
            .map(|s| StudentSearchItem {
                id: s.id,
                name: s.full_name,
                nis: s.nis.unwrap_or_else(|| "-".into()),
                class_name: s.class_name.unwrap_or_else(|| "-".into()),
            })
            .collect());
    }
    super::parent::search_students(pool, q).await
}

/// Pasang/ubah pengajar sebuah sesi (0 = kosongkan).
pub async fn set_session_teacher(pool: &Pool, session_id: i64, teacher_id: i64) -> Result<()> {
    let tid = (teacher_id > 0).then_some(teacher_id);
    if !repo::set_session_teacher(pool, session_id, tid).await? {
        bail!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Tandai sesi libur (cancelled) atau aktifkan kembali (scheduled).
pub async fn set_session_libur(pool: &Pool, session_id: i64, libur: bool) -> Result<()> {
    let status = if libur { "cancelled" } else { "scheduled" };
    if !repo::set_session_status(pool, session_id, status).await? {
        bail!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Payload halaman Students: daftar santri + antrean verifikasi sesuai peran
/// (pamong → tahap 1, dewan guru → tahap 2, admin → tahap 2, guru → tanpa antrean).
pub async fn students_data(pool: &Pool, user: &SessionUser) -> Result<StudentsData> {
    let board = repo::students_with_classes(pool, 300).await?;
    let students = board
        .into_iter()
        .map(|r| {
            let nis = r.nis.unwrap_or_default();
            StudentRowItem {
                initial: initial_of(&r.name),
                angkatan: angkatan_from_nis(&nis),
                nis: if nis.is_empty() { "-".into() } else { nis },
                classes: r
                    .classes
                    .into_iter()
                    .map(|c| StudentClassTag { golongan: c.golongan, name: c.name })
                    .collect(),
                points: r.points,
                id: r.user_id,
                name: r.name,
            }
        })
        .collect();

    let (verify_stage, pending_rows, verified_today) = match user.role.as_str() {
        "supervisor" => {
            let (p, cnt) =
                tokio::join!(repo::pending_pamong(pool, 50), repo::approved_today(pool));
            ("tahap1", p?, cnt?)
        }
        "dewan_guru" | "admin" => {
            let (p, cnt) =
                tokio::join!(repo::pending_verify(pool, 50), repo::verified_today(pool));
            ("tahap2", p?, cnt?)
        }
        _ => ("none", Vec::new(), 0),
    };

    let pending = pending_rows
        .into_iter()
        .map(|p| PendingAtt {
            id: p.id,
            name: p.full_name,
            nis: p.nis.unwrap_or_else(|| "-".into()),
            class_name: p.class_name.unwrap_or_else(|| "-".into()),
            time_label: fmt_when(p.scanned_at),
            gate: p.gate_label.unwrap_or_else(|| "-".into()),
        })
        .collect();

    Ok(StudentsData {
        role: user.role.clone(),
        verify_stage: verify_stage.to_string(),
        students,
        pending,
        verified_today,
    })
}

// ── Kurikulum (migrasi 17) ───────────────────────────────────────────────────

fn norm_status(status: &str) -> &'static str {
    match status {
        "completed" => "completed",
        "upcoming" => "upcoming",
        _ => "active",
    }
}

fn parse_progress(s: &str) -> Result<i16> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(0);
    }
    let n: i16 = s.parse().map_err(|_| anyhow::anyhow!("Progres harus berupa angka 0-100."))?;
    if !(0..=100).contains(&n) {
        bail!("Progres harus di antara 0 sampai 100.");
    }
    Ok(n)
}

#[allow(clippy::too_many_arguments)]
pub async fn create_curriculum(
    pool: &Pool,
    class_id: i64,
    title: &str,
    description: &str,
    scope_start: &str,
    scope_end: &str,
    progress_pct: &str,
    status: &str,
    book_id: i64,
) -> Result<i64> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Judul materi/kitab wajib diisi.");
    }
    let pct = parse_progress(progress_pct)?;
    repo::create_curriculum(
        pool,
        class_id,
        title,
        description.trim(),
        scope_start.trim(),
        scope_end.trim(),
        pct,
        norm_status(status),
        (book_id > 0).then_some(book_id),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn update_curriculum(
    pool: &Pool,
    id: i64,
    title: &str,
    description: &str,
    scope_start: &str,
    scope_end: &str,
    progress_pct: &str,
    status: &str,
    book_id: i64,
) -> Result<()> {
    let title = title.trim();
    if title.is_empty() {
        bail!("Judul materi/kitab wajib diisi.");
    }
    let pct = parse_progress(progress_pct)?;
    if !repo::update_curriculum(
        pool,
        id,
        title,
        description.trim(),
        scope_start.trim(),
        scope_end.trim(),
        pct,
        norm_status(status),
        (book_id > 0).then_some(book_id),
    )
    .await?
    {
        bail!("Materi kurikulum tidak ditemukan.");
    }
    Ok(())
}

pub async fn delete_curriculum(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_curriculum(pool, id).await? {
        bail!("Materi kurikulum tidak ditemukan.");
    }
    Ok(())
}
