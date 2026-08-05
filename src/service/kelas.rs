//! service/kelas.rs — Manajemen kelas (admin/guru/dewan guru/pamong): daftar
//! kelas, detail (anggota + jadwal + sesi), buat/ubah kelas, kategori fleksibel,
//! jadwal (buat/ubah/hapus + generate sesi bulanan), tambah/keluarkan santri,
//! serta payload halaman Students (daftar santri + antrean verifikasi per-peran).

use anyhow::Result;
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
            bail_user!(
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
        bail_user!("Poin {field} harus di antara 0 sampai 100 (tanpa minus).");
    }
    Ok(Some(n))
}

/// Jenis kegiatan PRD valid → Some(kanonik); selain itu (termasuk kosong) → None
/// (legacy preset). Menentukan preset poin default (models::category_points).
fn normalize_activity_type(s: &str) -> Option<String> {
    let s = s.trim();
    crate::models::ACTIVITY_TYPES
        .iter()
        .any(|(k, _)| *k == s)
        .then(|| s.to_string())
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

// ── Rentang & posisi materi (migrasi 57) ─────────────────────────────────────

/// `books.surahs` (JSONB) → daftar surat. Bentuk tak dikenal → kosong, bukan
/// galat: materi yang datanya aneh lebih baik tampil tanpa nama surat daripada
/// menggagalkan seluruh halaman kelas.
fn surahs_of(v: Option<&serde_json::Value>) -> Vec<crate::models::Surah> {
    v.and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default()
}

/// Nama surat ke-`idx` (1-based). Di luar jangkauan → "Surat {idx}" supaya
/// tetap terbaca ketimbang kosong.
fn surah_name(surahs: &[crate::models::Surah], idx: i32) -> String {
    usize::try_from(idx)
        .ok()
        .filter(|i| *i >= 1)
        .and_then(|i| surahs.get(i - 1))
        .map(|s| s.name.clone())
        .unwrap_or_else(|| format!("Surat {idx}"))
}

/// Satu titik posisi: "Halaman 42" (hadist) / "Al Baqarah ayat 120" (quran).
fn titik_label(
    category: &str,
    surahs: &[crate::models::Surah],
    surah: Option<i16>,
    unit: Option<i32>,
) -> String {
    let Some(u) = unit else { return String::new() };
    if category == "quran" {
        let s = surah.unwrap_or(1) as i32;
        format!("{} ayat {u}", surah_name(surahs, s))
    } else {
        format!("Halaman {u}")
    }
}

/// Rentang siap-tampil. Kosong-nya rentang BUKAN kesalahan: itu berarti
/// seluruh materi dipakai, dan mengatakannya lebih jujur daripada bidang kosong.
fn range_label(
    category: &str,
    surahs: &[crate::models::Surah],
    r: &repo::CurriculumRange,
) -> String {
    match (r.start_unit, r.end_unit) {
        (None, None) => "Seluruh materi".into(),
        _ => {
            let a = titik_label(category, surahs, r.start_surah, r.start_unit);
            let b = titik_label(category, surahs, r.end_surah, r.end_unit);
            match (a.is_empty(), b.is_empty()) {
                (false, false) if a == b => a,
                (false, false) => format!("{a} – {b}"),
                (false, true) => format!("Mulai {a}"),
                (true, false) => format!("Sampai {b}"),
                (true, true) => String::new(),
            }
        }
    }
}

/// Posisi sebagai SATU angka berurutan dari awal materi.
///
/// Perlu karena posisi Qur'an berdimensi dua (surat + ayat) sehingga tak bisa
/// dikurangkan langsung: "surat 2 ayat 10" dikurangi "surat 1 ayat 5" tak ada
/// artinya. Diratakan jadi nomor ayat kumulatif — ayat-ayat surat sebelumnya
/// dijumlahkan dulu — barulah selisihnya bermakna. Hadist sudah satu dimensi
/// (halaman), jadi angkanya dipakai apa adanya.
fn unit_absolut(
    category: &str,
    surahs: &[crate::models::Surah],
    surah: i32,
    unit: i32,
) -> i32 {
    if category != "quran" {
        return unit;
    }
    let idx = surah.max(1) as usize;
    let sebelumnya: i32 = surahs.iter().take(idx.saturating_sub(1)).map(|s| s.ayat).sum();
    sebelumnya + unit
}

/// Jumlah unit seluruh materi (total ayat semua surat, atau total halaman).
fn total_unit(category: &str, surahs: &[crate::models::Surah], total_pages: i32) -> i32 {
    if category == "quran" {
        surahs.iter().map(|s| s.ayat).sum()
    } else {
        total_pages
    }
}

/// Progres kurikulum DIHITUNG dari posisi yang sedang berjalan, bukan diketik.
///
/// Angka yang diketik tangan cepat basi: pengelola memperbarui posisi materi di
/// jadwal tapi lupa menyetel persennya, lalu dua tempat itu bercerita berbeda.
/// Di sini persennya diturunkan dari satu-satunya fakta yang memang dirawat —
/// sampai ayat/halaman berapa materinya berjalan.
///
/// `posisi` None (jadwal belum menandai materi ini) → 0%.
fn progres_dari_posisi(
    category: &str,
    surahs: &[crate::models::Surah],
    total_pages: i32,
    r: &repo::CurriculumRange,
    posisi: Option<i32>,
) -> i16 {
    let Some(pos) = posisi else { return 0 };
    // Rentang kosong = seluruh materi.
    let awal = r
        .start_unit
        .map(|u| unit_absolut(category, surahs, r.start_surah.unwrap_or(1) as i32, u))
        .unwrap_or(1);
    let akhir = r
        .end_unit
        .map(|u| unit_absolut(category, surahs, r.end_surah.unwrap_or(1) as i32, u))
        .unwrap_or_else(|| total_unit(category, surahs, total_pages));
    let panjang = akhir - awal + 1;
    if panjang <= 0 {
        return 0;
    }
    let maju = (pos - awal + 1).clamp(0, panjang);
    ((maju as f64 / panjang as f64) * 100.0).round().clamp(0.0, 100.0) as i16
}

/// Periksa rentang terhadap materi yang ditunjuk, lalu kembalikan bentuk yang
/// siap disimpan.
///
/// Batas ATAS tak bisa dicek di CHECK constraint (bergantung materi mana yang
/// ditunjuk), jadi di sinilah tempatnya — di satu fungsi yang dipakai bersama
/// pembuatan maupun penyuntingan, supaya aturannya tak jadi dua salinan.
///
/// Rentang kosong = seluruh materi, itu sah. Yang ditolak adalah rentang yang
/// separuh terisi atau melampaui materinya.
/// Periksa satu POSISI (surat+ayat / halaman) terhadap materinya.
///
/// Dipakai dua tempat dengan arti berbeda tapi aturan sama: posisi milik
/// jadwal ("jadwal ini sampai mana") dan posisi milik kurikulum ("kelas ini
/// sampai mana"). Batas atasnya bergantung materi, jadi tak bisa jadi CHECK
/// constraint — di sinilah tempatnya, satu salinan untuk keduanya.
///
/// `unit` 0 = belum diisi, itu sah.
async fn periksa_posisi(
    pool: &Pool,
    book_id: i64,
    surah: i32,
    unit: i32,
) -> Result<(Option<i16>, Option<i32>)> {
    if unit <= 0 {
        return Ok((None, None));
    }
    let Some(book) = repo::get_book(pool, book_id).await? else {
        bail_user!("Materi yang dipilih tidak ditemukan.");
    };
    posisi_dalam_materi(&book, surah, unit)
}

/// Inti pemeriksaan posisi, dipakai [`periksa_posisi`] & [`periksa_rentang`]
/// (yang materinya sudah terlanjur diambil, jadi tak perlu query ulang).
fn posisi_dalam_materi(
    book: &repo::BookRow,
    surah: i32,
    unit: i32,
) -> Result<(Option<i16>, Option<i32>)> {
    if unit <= 0 {
        return Ok((None, None));
    }
    if book.category == "quran" {
        let surahs = surahs_of(Some(&book.surahs));
        let n = surahs.len() as i32;
        let cs = surah.max(1);
        if cs > n {
            bail_user!("Surat posisi di luar materi ini (hanya ada {n} surat).");
        }
        let batas = surahs[(cs - 1) as usize].ayat;
        if unit > batas {
            bail_user!("{} hanya sampai ayat {}.", surah_name(&surahs, cs), batas);
        }
        return Ok((Some(cs as i16), Some(unit)));
    }
    if unit > book.total_pages {
        bail_user!("Materi ini hanya {} halaman.", book.total_pages);
    }
    Ok((None, Some(unit)))
}

/// Mengembalikan (judul materi, rentang) — judulnya dipakai mengisi
/// `curriculum.title` supaya kurikulum tak perlu mengetik judul sendiri.
#[allow(clippy::too_many_arguments)]
async fn periksa_rentang(
    pool: &Pool,
    book_id: i64,
    start_surah: i32,
    start_unit: i32,
    end_surah: i32,
    end_unit: i32,
    cur_surah: i32,
    cur_unit: i32,
) -> Result<(String, repo::CurriculumRange)> {
    let Some(book) = repo::get_book(pool, book_id).await? else {
        bail_user!("Materi yang dipilih tidak ditemukan.");
    };
    let judul = book.title.clone();
    let posisi = posisi_dalam_materi(&book, cur_surah, cur_unit)?;

    if start_unit == 0 && end_unit == 0 {
        return Ok((
            judul,
            repo::CurriculumRange {
                current_surah: posisi.0,
                current_unit: posisi.1,
                ..Default::default()
            },
        ));
    }
    if start_unit == 0 || end_unit == 0 {
        bail_user!("Isi kedua ujung rentang, atau kosongkan keduanya untuk seluruh materi.");
    }

    if book.category == "quran" {
        let surahs = surahs_of(Some(&book.surahs));
        if surahs.is_empty() {
            bail_user!("Materi Qur'an ini belum punya daftar surat, jadi rentangnya tak bisa diisi.");
        }
        let n = surahs.len() as i32;
        let (ss, es) = (start_surah.max(1), end_surah.max(1));
        if ss > n || es > n {
            bail_user!("Surat yang dipilih di luar materi ini (hanya ada {n} surat).");
        }
        let batas = |i: i32| surahs[(i - 1) as usize].ayat;
        if start_unit > batas(ss) {
            bail_user!("{} hanya sampai ayat {}.", surah_name(&surahs, ss), batas(ss));
        }
        if end_unit > batas(es) {
            bail_user!("{} hanya sampai ayat {}.", surah_name(&surahs, es), batas(es));
        }
        // Bandingkan sebagai pasangan (surat, ayat) — rentang boleh melintasi
        // surat, jadi membandingkan ayatnya saja akan salah.
        if (ss, start_unit) > (es, end_unit) {
            bail_user!("Awal rentang harus sebelum akhirnya.");
        }
        return Ok((
            judul,
            repo::CurriculumRange {
                start_surah: Some(ss as i16),
                start_unit: Some(start_unit),
                end_surah: Some(es as i16),
                end_unit: Some(end_unit),
                current_surah: posisi.0,
                current_unit: posisi.1,
            },
        ));
    }

    // Hadist: halaman, tanpa surat.
    if start_unit > end_unit {
        bail_user!("Halaman awal harus sebelum halaman akhir.");
    }
    if end_unit > book.total_pages {
        bail_user!("Materi ini hanya {} halaman.", book.total_pages);
    }
    Ok((
        judul,
        repo::CurriculumRange {
            start_surah: None,
            start_unit: Some(start_unit),
            end_surah: None,
            end_unit: Some(end_unit),
            current_surah: posisi.0,
            current_unit: posisi.1,
        },
    ))
}

/// Status kurikulum DITURUNKAN dari progres, bukan dipilih tangan.
///
/// Dulu persen dan status dua isian terpisah: seseorang bisa menandai "Selesai"
/// padahal progresnya 40%, atau materinya sudah khatam tapi statusnya masih
/// "Berjalan" karena lupa diubah. Sekarang keduanya turunan dari satu angka
/// yang sama — posisi terakhir — jadi mustahil bertentangan.
fn status_dari_progres(pct: i16, sudah_mulai: bool) -> &'static str {
    if pct >= 100 {
        "completed"
    } else if sudah_mulai {
        "active"
    } else {
        "upcoming"
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
    let Some(ci) = repo::class_info(pool, class_id).await? else {
        bail_user!("Kelas tidak ditemukan.");
    };

    // CATATAN PERF: materialisasi sesi (menulis) TIDAK lagi di sini — dulu tiap
    // GET detail menulis sesi (serial per-jadwal) → lambat, apalagi DB remote.
    // Kini dilakukan: (1) saat BUAT jadwal, (2) task background 600s (semua
    // kelas) di main.rs. Halaman detail = murni baca (5 query paralel).
    // Sesi yang DITAMPILKAN hanya MULAI hari ini ke depan (yang lewat dibuang).
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let (members, scheds, sessions, teachers, cats, golongans, curriculum, books, rooms, pamongs) = tokio::join!(
        repo::class_members(pool, class_id),
        repo::class_schedules(pool, class_id),
        repo::sessions_of_class(pool, class_id, today, 50),
        repo::teacher_options(pool),
        repo::distinct_categories(pool),
        repo::distinct_golongan(pool),
        repo::class_curriculum(pool, class_id),
        repo::list_books(pool),
        repo::device_options(pool),
        repo::pamong_options(pool),
    );
    let pamong_options: Vec<crate::models::TeacherOption> = pamongs?
        .into_iter()
        .map(|(id, name)| crate::models::TeacherOption { id, name })
        .collect();

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
    let cur_rows = curriculum?;

    let schedules = scheds
        .into_iter()
        .map(|s| {
            let cur_cat = s.current_book_category.clone().unwrap_or_default();
            let cur_surahs = surahs_of(s.current_book_surahs.as_ref());
            let current_label =
                titik_label(&cur_cat, &cur_surahs, s.current_surah, s.current_unit);
            ScheduleItem {
            current_book_id: s.current_book_id.unwrap_or(0),
            current_book_title: s.current_book_title.clone().unwrap_or_default(),
            current_book_category: cur_cat,
            current_surah: s.current_surah.unwrap_or(0) as i32,
            current_unit: s.current_unit.unwrap_or(0),
            current_label,
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
            activity_type: s.activity_type.clone().unwrap_or_default(),
            izin_points: s.izin_points.map(|n| n.to_string()).unwrap_or_default(),
            }
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
                pamong_id: r.pamong_id,
                category: r.category.filter(|c| !c.is_empty()).unwrap_or_else(|| "-".into()),
            }
        })
        .collect();

    let teacher_options = teachers?
        .into_iter()
        .map(|(id, name)| TeacherOption { id, name })
        .collect();

    let curriculum = cur_rows
        .into_iter()
        .map(|c| {
            let category = c.book_category.clone().unwrap_or_default();
            let surahs = surahs_of(c.book_surahs.as_ref());
            let range = repo::CurriculumRange {
                start_surah: c.start_surah,
                start_unit: c.start_unit,
                end_surah: c.end_surah,
                end_unit: c.end_unit,
                current_surah: c.current_surah,
                current_unit: c.current_unit,
            };
            // Cakupan teks-bebas lama sudah dibuang (migrasi 58) — semua baris
            // kini bersandar pada materi + rentang angkanya.
            let range_label = range_label(&category, &surahs, &range);
            // Posisi milik baris kurikulum ini sendiri (migrasi 59). Dari SATU
            // angka ini persen dan status sama-sama diturunkan.
            let posisi = c.current_unit.map(|u| {
                unit_absolut(&category, &surahs, c.current_surah.unwrap_or(1) as i32, u)
            });
            let progress_pct = progres_dari_posisi(
                &category,
                &surahs,
                c.book_total_pages.unwrap_or(0),
                &range,
                posisi,
            );
            let status_kode = status_dari_progres(progress_pct, posisi.is_some());
            let current_label =
                titik_label(&category, &surahs, c.current_surah, c.current_unit);
            CurriculumItem {
                id: c.id,
                // Judul mengikuti materinya; `curriculum.title` hanya dipakai
                // untuk baris lama yang belum tertaut.
                title: c.book_title.clone().unwrap_or(c.title),
                progress_pct,
                order_index: c.order_index,
                status_label: curriculum_status_label(status_kode).into(),
                status: status_kode.to_string(),
                book_id: c.book_id.unwrap_or(0),
                book_title: c.book_title.unwrap_or_default(),
                book_category: category,
                start_surah: range.start_surah.unwrap_or(0) as i32,
                start_unit: range.start_unit.unwrap_or(0),
                end_surah: range.end_surah.unwrap_or(0) as i32,
                end_unit: range.end_unit.unwrap_or(0),
                range_label,
                current_surah: c.current_surah.unwrap_or(0) as i32,
                current_unit: c.current_unit.unwrap_or(0),
                current_label,
            }
        })
        .collect();

    let book_options = books?
        .into_iter()
        // Daftar surat IKUT dikirim (dulu selalu kosong): form kurikulum &
        // jadwal memakainya untuk menyusun pilihan surat dan batas ayatnya.
        .map(|b| crate::models::BookItem {
            id: b.id,
            title: b.title,
            category: b.category,
            total_pages: b.total_pages,
            surahs: surahs_of(Some(&b.surahs)),
        })
        .collect();
    let room_options = rooms?
        .into_iter()
        .map(|(id, name)| crate::models::RoomOption { id, name })
        .collect();

    Ok(KelasDetail {
        role: role.to_string(),
        id: class_id,
        name: ci.name,
        description: ci.description,
        category: ci.category.unwrap_or_default(),
        category_options: cats?,
        golongan: ci.golongan.unwrap_or_default(),
        golongan_options: golongans?,
        wali_kelas_id: ci.wali_kelas_id.unwrap_or(0),
        wali_kelas_name: ci.wali_kelas_name.unwrap_or_default(),
        require_pamong: ci.require_pamong,
        pamong_id: ci.pamong_id.unwrap_or(0),
        pamong_name: ci.pamong_name.unwrap_or_default(),
        pamong_options,
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
        bail_user!("Nama kelas wajib diisi.");
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
        bail_user!("Nama kelas wajib diisi.");
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
        bail_user!("Kelas tidak ditemukan.");
    }
    Ok(())
}

pub async fn categories(pool: &Pool) -> Result<Vec<String>> {
    repo::distinct_categories(pool).await
}

/// Tetapkan wali kelas + pamong + rute persetujuan izin (require_pamong) satu
/// kelas (migrasi 29/30). id 0 = kosongkan.
pub async fn set_class_staff(
    pool: &Pool,
    class_id: i64,
    wali_kelas_id: i64,
    pamong_id: i64,
    require_pamong: bool,
) -> Result<()> {
    let wali = (wali_kelas_id > 0).then_some(wali_kelas_id);
    let pamong = (pamong_id > 0).then_some(pamong_id);
    if !repo::set_class_staff(pool, class_id, wali, pamong, require_pamong).await? {
        bail_user!("Kelas tidak ditemukan.");
    }
    Ok(())
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
        bail_user!("Jam selesai harus setelah jam mulai.");
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
    activity_type: &str,
    izin_points: &str,
) -> Result<i64> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    // Recurrence 'custom' = tanggal manual (loncat-loncat): start/end jadwal
    // diturunkan dari min/max tanggal, bukan dari form.
    let custom = if recurrence == "custom" { parse_custom_dates(custom_dates)? } else { Vec::new() };
    if recurrence == "custom" && custom.is_empty() {
        bail_user!("Pilih minimal satu tanggal untuk jadwal tanggal-tertentu.");
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
    let ip = parse_point_magnitude(izin_points, "izin")?;
    let atype = normalize_activity_type(activity_type);
    let room = (room_id > 0).then_some(room_id);
    // Dropdown sudah tak menawarkannya, tapi server fn bisa dipanggil langsung
    // dengan id apa pun — tolak di sini juga. Jadwal beruang gerbang utama
    // TIDAK AKAN PERNAH bisa diabsen (tap di gerbang cuma toggle keluar/masuk).
    if let Some(rid) = room {
        if repo::is_gate_device(pool, rid).await? {
            bail_user!(
                "Gerbang utama tidak bisa dipakai sebagai ruang kelas — tap di sana \
                 hanya menandai keluar/masuk pondok, bukan kehadiran kelas. \
                 Kosongkan ruang bila kelas ini boleh diabsen di mana saja."
            );
        }
    }
    let cd_json = custom_dates_json(&custom);
    let id = repo::create_schedule(
        pool, class_id, title.trim(), st, et, lt, rec, sd, ed, cat, pp, lp, ap, room, &cd_json,
        atype.as_deref(), ip,
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
    activity_type: &str,
    izin_points: &str,
) -> Result<()> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    let custom = if recurrence == "custom" { parse_custom_dates(custom_dates)? } else { Vec::new() };
    if recurrence == "custom" && custom.is_empty() {
        bail_user!("Pilih minimal satu tanggal untuk jadwal tanggal-tertentu.");
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
    let ip = parse_point_magnitude(izin_points, "izin")?;
    let atype = normalize_activity_type(activity_type);
    let room = (room_id > 0).then_some(room_id);
    // Dropdown sudah tak menawarkannya, tapi server fn bisa dipanggil langsung
    // dengan id apa pun — tolak di sini juga. Jadwal beruang gerbang utama
    // TIDAK AKAN PERNAH bisa diabsen (tap di gerbang cuma toggle keluar/masuk).
    if let Some(rid) = room {
        if repo::is_gate_device(pool, rid).await? {
            bail_user!(
                "Gerbang utama tidak bisa dipakai sebagai ruang kelas — tap di sana \
                 hanya menandai keluar/masuk pondok, bukan kehadiran kelas. \
                 Kosongkan ruang bila kelas ini boleh diabsen di mana saja."
            );
        }
    }
    let cd_json = custom_dates_json(&custom);
    if !repo::update_schedule(
        pool, schedule_id, title.trim(), st, et, lt, rec, sd, ed, cat, pp, lp, ap, room, &cd_json,
        atype.as_deref(), ip,
    )
    .await?
    {
        bail_user!("Jadwal tidak ditemukan.");
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
        bail_user!("Jadwal tidak ditemukan.");
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
        bail_user!("Bulan tidak valid.");
    }
    let Some((class_id, title, rec, start_date)) = repo::schedule_info(pool, schedule_id).await?
    else {
        bail_user!("Jadwal tidak ditemukan.");
    };
    let Some(first) = NaiveDate::from_ymd_opt(year, month, 1) else {
        bail_user!("Bulan/tahun tidak valid.");
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
                bail_user!("Buku tidak ditemukan.");
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
    // Materi sesi wajib berasal dari kurikulum kelasnya. Dropdown sudah
    // disaring, tapi request bisa dikirim langsung.
    if let Some(b) = book {
        if !repo::session_book_in_curriculum(pool, session_id, b).await? {
            bail_user!(
                "Materi itu belum ada di kurikulum kelas ini. Tambahkan dulu lewat detail kelas."
            );
        }
    }
    let pages = book_pages_value(pool, book, book_pages_text).await?;
    if !repo::set_session_book(pool, session_id, book, &pages).await? {
        bail_user!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Set materi TARGET/rencana sesi (migrasi 41). book_id 0 = kosongkan.
pub async fn set_session_target(
    pool: &Pool,
    session_id: i64,
    book_id: i64,
    pages_text: &str,
) -> Result<()> {
    let book = Some(book_id).filter(|v| *v > 0);
    // Materi sesi wajib berasal dari kurikulum kelasnya. Dropdown sudah
    // disaring, tapi request bisa dikirim langsung.
    if let Some(b) = book {
        if !repo::session_book_in_curriculum(pool, session_id, b).await? {
            bail_user!(
                "Materi itu belum ada di kurikulum kelas ini. Tambahkan dulu lewat detail kelas."
            );
        }
    }
    let pages = book_pages_value(pool, book, pages_text).await?;
    if !repo::set_session_target(pool, session_id, book, &pages).await? {
        bail_user!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Set catatan ayat/hadith AKTUAL sesi (teks bebas, maks 500). Kosong diperbolehkan.
pub async fn set_session_actual_detail(pool: &Pool, session_id: i64, detail: &str) -> Result<()> {
    let d = detail.trim();
    if d.chars().count() > 500 {
        bail_user!("Catatan maksimal 500 karakter.");
    }
    if !repo::set_session_actual_detail(pool, session_id, d).await? {
        bail_user!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Tambah santri ke KELAS. Keanggotaan berlaku untuk SEMUA jadwal kelas itu
/// (migrasi 61) — tak ada lagi penempatan per-jadwal.
pub async fn add_member(pool: &Pool, class_id: i64, student_id: i64) -> Result<()> {
    if !repo::add_member(pool, class_id, student_id).await? {
        bail_user!("Santri sudah terdaftar di kelas ini.");
    }
    Ok(())
}

/// Tambah BANYAK santri ke kelas sekaligus. Return jumlah BARU ditambahkan.
pub async fn add_members(
    pool: &Pool,
    class_id: i64,
    student_ids: Vec<i64>,
) -> Result<i64> {
    let ids: Vec<i64> = student_ids.into_iter().filter(|&x| x > 0).collect();
    if ids.is_empty() {
        bail_user!("Pilih minimal satu santri.");
    }
    repo::add_members(pool, class_id, &ids).await
}

pub async fn remove_member(pool: &Pool, class_id: i64, student_id: i64) -> Result<()> {
    if !repo::remove_member(pool, class_id, student_id).await? {
        bail_user!("Santri tidak ada di kelas ini.");
    }
    Ok(())
}

/// Cari santri untuk ditambahkan ke `class_id`. MENGECUALIKAN santri yang sudah
/// jadi anggota kelas itu (tak perlu ditambah lagi). Query pendek/kosong →
/// daftar DEFAULT supaya form tak kosong sebelum mengetik.
pub async fn search_students(pool: &Pool, q: &str, class_id: i64) -> Result<Vec<StudentSearchItem>> {
    Ok(repo::students_not_in_class(pool, class_id, q, 20)
        .await?
        .into_iter()
        .map(|s| StudentSearchItem {
            id: s.id,
            name: s.full_name,
            nis: s.nis.unwrap_or_else(|| "-".into()),
            class_name: s.class_name.unwrap_or_else(|| "-".into()),
        })
        .collect())
}

/// Pasang/ubah pengajar sebuah sesi (0 = kosongkan).
pub async fn set_session_teacher(pool: &Pool, session_id: i64, teacher_id: i64) -> Result<()> {
    let tid = (teacher_id > 0).then_some(teacher_id);
    if !repo::set_session_teacher(pool, session_id, tid).await? {
        bail_user!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Set pamong bertugas verifikasi satu sesi (migrasi 33). 0 = kosongkan.
pub async fn set_session_pamong(pool: &Pool, session_id: i64, pamong_id: i64) -> Result<()> {
    let pid = (pamong_id > 0).then_some(pamong_id);
    if !repo::set_session_pamong(pool, session_id, pid).await? {
        bail_user!("Sesi tidak ditemukan.");
    }
    Ok(())
}

/// Tandai sesi libur (cancelled) atau aktifkan kembali (scheduled).
pub async fn set_session_libur(pool: &Pool, session_id: i64, libur: bool) -> Result<()> {
    let status = if libur { "cancelled" } else { "scheduled" };
    if !repo::set_session_status(pool, session_id, status).await? {
        bail_user!("Sesi tidak ditemukan.");
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
        // Pamong bertugas → tahap 1 (hanya sesi yang ia tugaskan, migrasi 33).
        "supervisor" => {
            let (p, cnt) = tokio::join!(
                repo::pending_pamong(pool, Some(user.id), 50),
                repo::approved_today(pool)
            );
            ("tahap1", p?, cnt?)
        }
        // Ustad bertugas → tahap FINAL (hanya sesi yang ia ampu).
        "teacher" => {
            let (p, cnt) = tokio::join!(
                repo::pending_verify(pool, Some(user.id), 50),
                repo::verified_today(pool)
            );
            ("tahap2", p?, cnt?)
        }
        // Dewan guru/admin → tahap FINAL semua sesi (oversight).
        "dewan_guru" | "admin" => {
            let (p, cnt) =
                tokio::join!(repo::pending_verify(pool, None, 50), repo::verified_today(pool));
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



/// Tambah materi ke kurikulum kelas.
///
/// Kurikulum TIDAK lagi punya judul/deskripsi/cakupan teksnya sendiri: semua
/// itu sudah ada di materinya (`books`), dan menyalinnya ke sini berarti dua
/// tempat yang bisa berbeda isinya. Yang disimpan kurikulum hanyalah TAUTAN ke
/// materi + rentang halaman/ayat + progres + status.
///
/// `curriculum.title` (kolom NOT NULL warisan migrasi 17) diisi otomatis dari
/// judul materi supaya tetap terbaca oleh query lama, bukan diketik pengguna.
#[allow(clippy::too_many_arguments)]
pub async fn create_curriculum(
    pool: &Pool,
    class_id: i64,
    book_id: i64,
    start_surah: i32,
    start_unit: i32,
    end_surah: i32,
    end_unit: i32,
    cur_surah: i32,
    cur_unit: i32,
) -> Result<i64> {
    if book_id <= 0 {
        bail_user!("Pilih materi terdaftar untuk kurikulum ini.");
    }
    let (judul, range) =
        periksa_rentang(pool, book_id, start_surah, start_unit, end_surah, end_unit, cur_surah, cur_unit)
            .await?;
    repo::create_curriculum(
        pool,
        class_id,
        &judul,
        Some(book_id),
        range,
    )
    .await
}

/// Ubah satu baris kurikulum. Aturan & bentuknya sama dengan
/// [`create_curriculum`] — judul tetap ikut materinya, bukan diketik ulang.
#[allow(clippy::too_many_arguments)]
pub async fn update_curriculum(
    pool: &Pool,
    id: i64,
    book_id: i64,
    start_surah: i32,
    start_unit: i32,
    end_surah: i32,
    end_unit: i32,
    cur_surah: i32,
    cur_unit: i32,
) -> Result<()> {
    if book_id <= 0 {
        bail_user!("Pilih materi terdaftar untuk kurikulum ini.");
    }
    let (judul, range) =
        periksa_rentang(pool, book_id, start_surah, start_unit, end_surah, end_unit, cur_surah, cur_unit)
            .await?;
    if !repo::update_curriculum(
        pool,
        id,
        &judul,
        Some(book_id),
        range,
    )
    .await?
    {
        bail_user!("Materi kurikulum tidak ditemukan.");
    }
    Ok(())
}

/// Setel materi & posisi yang SEDANG BERJALAN pada satu jadwal (migrasi 57).
///
/// `book_id` 0 = lepaskan penanda. Posisi divalidasi terhadap materinya persis
/// seperti rentang kurikulum — dipakai ulang lewat [`periksa_rentang`] dengan
/// awal = akhir, supaya "ayat 300 di surat berayat 286" ditolak di sini juga,
/// bukan hanya di form.
pub async fn set_schedule_current(
    pool: &Pool,
    schedule_id: i64,
    book_id: i64,
    surah: i32,
    unit: i32,
) -> Result<()> {
    if book_id <= 0 {
        if !repo::set_schedule_current(pool, schedule_id, None, None, None).await? {
            bail_user!("Jadwal tidak ditemukan.");
        }
        return Ok(());
    }
    if !repo::schedule_book_in_curriculum(pool, schedule_id, book_id).await? {
        bail_user!("Materi itu belum ada di kurikulum kelas ini. Tambahkan dulu di tab Kurikulum.");
    }
    let (s, u) = periksa_posisi(pool, book_id, surah, unit).await?;
    if !repo::set_schedule_current(pool, schedule_id, Some(book_id), s, u).await? {
        bail_user!("Jadwal tidak ditemukan.");
    }
    Ok(())
}

pub async fn delete_curriculum(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_curriculum(pool, id).await? {
        bail_user!("Materi kurikulum tidak ditemukan.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_activity_type_valid_saja() {
        assert_eq!(normalize_activity_type("kbm").as_deref(), Some("kbm"));
        assert_eq!(normalize_activity_type("non_kbm").as_deref(), Some("non_kbm"));
        assert_eq!(normalize_activity_type("piket").as_deref(), Some("piket"));
        assert_eq!(normalize_activity_type("apel_kepulangan").as_deref(), Some("apel_kepulangan"));
        assert_eq!(normalize_activity_type("  kbm  ").as_deref(), Some("kbm"));
        // Tak valid / kosong / salah huruf → None (legacy).
        assert_eq!(normalize_activity_type("KBM"), None);
        assert_eq!(normalize_activity_type(""), None);
        assert_eq!(normalize_activity_type("ngaji"), None);
    }

    #[test]
    fn parse_point_magnitude_batas() {
        assert_eq!(parse_point_magnitude("", "x").unwrap(), None);
        assert_eq!(parse_point_magnitude("  ", "x").unwrap(), None);
        assert_eq!(parse_point_magnitude("0", "x").unwrap(), Some(0));
        assert_eq!(parse_point_magnitude("10", "x").unwrap(), Some(10));
        assert_eq!(parse_point_magnitude("100", "x").unwrap(), Some(100));
        // Di luar 0..=100 atau bukan angka positif → error.
        assert!(parse_point_magnitude("101", "x").is_err());
        assert!(parse_point_magnitude("-5", "x").is_err());
        assert!(parse_point_magnitude("abc", "x").is_err());
    }
}

// ── Sisi SANTRI: "Kelas Saya" ────────────────────────────────────────────────

/// Kelas dilihat dari sisi orang di dalamnya, lengkap dengan kurikulum, materi
/// yang sedang berjalan, petugas, dan daftar santri.
///
/// `sebagai_staf` menentukan kelas MANA yang diambil: santri → kelas yang ia
/// ikuti; wali kelas/pamong → kelas yang ia pegang. Isi kartunya identik, jadi
/// pemetaannya dipakai bersama alih-alih ditulis dua kali.
///
/// Query per-kelas (kurikulum/jadwal/anggota) sengaja dibiarkan berurutan:
/// seorang santri lazimnya ikut 2–3 kelas (satu per golongan), jadi jumlah
/// query tetap kecil dan menukarnya dengan satu query raksasa ber-JOIN ganda
/// justru lebih sulit dibaca tanpa keuntungan nyata.
pub async fn kelas_saya(
    pool: &Pool,
    user_id: i64,
    sebagai_staf: bool,
) -> Result<crate::models::KelasSayaData> {
    // Sumber kelasnya yang berbeda; isi kartunya sama persis.
    let kelas = if sebagai_staf {
        repo::classes_of_staff(pool, user_id).await?
    } else {
        repo::classes_of_student(pool, user_id).await?
    };
    let mut items = Vec::with_capacity(kelas.len());

    for k in kelas {
        let (cur_rows, sched_rows, members) = tokio::join!(
            repo::class_curriculum(pool, k.id),
            repo::class_schedules(pool, k.id),
            repo::class_members(pool, k.id),
        );

        let curriculum = cur_rows?
            .into_iter()
            .map(|c| {
                let category = c.book_category.clone().unwrap_or_default();
                let surahs = surahs_of(c.book_surahs.as_ref());
                let range = repo::CurriculumRange {
                    start_surah: c.start_surah,
                    start_unit: c.start_unit,
                    end_surah: c.end_surah,
                    end_unit: c.end_unit,
                    current_surah: c.current_surah,
                    current_unit: c.current_unit,
                };
                let posisi = c.current_unit.map(|u| {
                    unit_absolut(&category, &surahs, c.current_surah.unwrap_or(1) as i32, u)
                });
                let progress_pct = progres_dari_posisi(
                    &category,
                    &surahs,
                    c.book_total_pages.unwrap_or(0),
                    &range,
                    posisi,
                );
                let status_kode = status_dari_progres(progress_pct, posisi.is_some());
                CurriculumItem {
                    id: c.id,
                    title: c.book_title.clone().unwrap_or(c.title),
                    progress_pct,
                    order_index: c.order_index,
                    status_label: curriculum_status_label(status_kode).into(),
                    status: status_kode.to_string(),
                    book_id: c.book_id.unwrap_or(0),
                    book_title: c.book_title.unwrap_or_default(),
                    book_category: category.clone(),
                    start_surah: range.start_surah.unwrap_or(0) as i32,
                    start_unit: range.start_unit.unwrap_or(0),
                    end_surah: range.end_surah.unwrap_or(0) as i32,
                    end_unit: range.end_unit.unwrap_or(0),
                    range_label: range_label(&category, &surahs, &range),
                    current_surah: range.current_surah.unwrap_or(0) as i32,
                    current_unit: range.current_unit.unwrap_or(0),
                    current_label: titik_label(
                        &category,
                        &surahs,
                        range.current_surah,
                        range.current_unit,
                    ),
                }
            })
            .collect();

        let schedules = sched_rows?
            .into_iter()
            .map(|s| {
                let cat = s.current_book_category.clone().unwrap_or_default();
                let surahs = surahs_of(s.current_book_surahs.as_ref());
                crate::models::KelasSayaJadwal {
                    title: if s.title.trim().is_empty() {
                        "Jadwal Kelas".into()
                    } else {
                        s.title.clone()
                    },
                    time_label: format!(
                        "{} – {} WIB",
                        s.start_time.format("%H:%M"),
                        s.end_time.format("%H:%M")
                    ),
                    recurrence_label: recurrence_label(&s.recurrence_type).into(),
                    current_book_title: s.current_book_title.clone().unwrap_or_default(),
                    current_label: titik_label(&cat, &surahs, s.current_surah, s.current_unit),
                }
            })
            .collect();

        let members = members?
            .into_iter()
            .map(|(id, name, nis)| {
                let nis = nis.unwrap_or_default();
                MemberItem {
                    angkatan: nis.chars().take(4).collect::<String>(),
                    id,
                    name,
                    nis,
                }
            })
            .collect();

        let peran_saya = match (k.saya_wali, k.saya_pamong) {
            (true, true) => "Wali Kelas & Pamong",
            (true, false) => "Wali Kelas",
            (false, true) => "Pamong",
            _ => "",
        };

        items.push(crate::models::KelasSayaItem {
            id: k.id,
            name: k.name,
            peran_saya: peran_saya.to_string(),
            category: k.category.unwrap_or_default(),
            golongan: k.golongan.unwrap_or_default(),
            wali_kelas: k.wali_kelas.unwrap_or_default(),
            pamong: k.pamong.unwrap_or_default(),
            curriculum,
            schedules,
            members,
        });
    }

    Ok(crate::models::KelasSayaData { sebagai_staf, items })
}
