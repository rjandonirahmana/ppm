//! service/santri.rs — Logika halaman santri: riwayat kehadiran, izin, profil.

use anyhow::{bail, Result};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use deadpool_postgres::Pool;

use super::fmt::{fmt_dt_full, fmt_month, fmt_range, wib};
use crate::models::{
    permit_kind_label, permit_stage, point_rule, IzinData, PermitItem, ProfilData, RiwayatData,
    RiwayatItem,
};
use crate::repository as repo;

/// Awal semester akademik (WIB): Juli–Des = Ganjil (mulai 1 Jul),
/// Jan–Jun = Genap (mulai 1 Jan). Return (awal_utc, label).
pub(crate) fn semester_start() -> (chrono::DateTime<Utc>, String) {
    let now = Utc::now().with_timezone(&wib());
    let (start, label) = if now.month() >= 7 {
        let y = now.year();
        (
            NaiveDate::from_ymd_opt(y, 7, 1).expect("1 Jul valid"),
            format!("Semester Ganjil {}/{}", y % 100, (y + 1) % 100),
        )
    } else {
        let y = now.year();
        (
            NaiveDate::from_ymd_opt(y, 1, 1).expect("1 Jan valid"),
            format!("Semester Genap {}/{}", (y - 1) % 100, y % 100),
        )
    };
    let start_utc = wib()
        .from_local_datetime(&start.and_hms_opt(0, 0, 0).expect("00:00 valid"))
        .single()
        .expect("awal semester valid")
        .with_timezone(&Utc);
    (start_utc, label)
}

/// Awal "semester berjalan" — dipakai async caller lain (signature tetap
/// `async fn` walau kini pass-through ke `semester_start()`).
pub(crate) async fn current_semester(_pool: &Pool) -> Result<(chrono::DateTime<Utc>, String)> {
    Ok(semester_start())
}

pub(crate) fn status_display(status: &str) -> (&'static str, &'static str) {
    match status {
        "present" => ("HADIR", "present"),
        "late" => ("TERLAMBAT", "late"),
        // Warna reuse "late" (oranye) — visual "irregular"; label dibedakan.
        "outside_schedule" => ("DI LUAR JADWAL", "late"),
        "permit" | "sick" => ("IZIN", "permit"),
        _ => ("ALPA", "absent"),
    }
}

/// Riwayat kehadiran lengkap + statistik semester.
pub async fn riwayat(pool: &Pool, user_id: i64) -> Result<RiwayatData> {
    let (since, semester_label) = current_semester(pool).await?;
    let (stats, rows) = tokio::join!(
        repo::semester_stats(pool, user_id, since),
        repo::riwayat_all(pool, user_id, 200),
    );
    let (hadir, izin, alpa, _total) = stats?;

    let items = rows?
        .into_iter()
        .map(|r| {
            let (status_label, kind) = status_display(&r.status);
            let (points, note, _) = point_rule(&r.status);
            let title = r
                .title
                .or_else(|| r.gate_label.clone().map(|g| format!("Absensi - {g}")))
                .unwrap_or_else(|| "Kehadiran".into());
            RiwayatItem {
                title,
                time_label: fmt_dt_full(r.scanned_at),
                status_label: status_label.into(),
                kind: kind.into(),
                points,
                points_note: note.into(),
                month: fmt_month(r.scanned_at),
            }
        })
        .collect();

    Ok(RiwayatData {
        hadir,
        izin,
        alpa,
        semester_label,
        items,
    })
}

/// Data halaman Ajukan Perizinan.
pub async fn izin_data(pool: &Pool, user_id: i64) -> Result<IzinData> {
    let (since, _) = current_semester(pool).await?;
    let today = Utc::now().with_timezone(&wib()).date_naive();

    let (stats, home, detected, permits) = tokio::join!(
        repo::semester_stats(pool, user_id, since),
        repo::user_home(pool, user_id),
        repo::latest_scan_today(pool, user_id, today),
        repo::list_my_permits(pool, user_id, 5),
    );

    let (hadir, _izin, alpa, total) = stats?;
    let pct = if total > 0 { ((hadir * 100) / total) as i32 } else { 0 };
    let points = home?.map(|h| h.points).unwrap_or(0);

    let detected = detected?.map(|(title, ts)| {
        let t = ts.with_timezone(&wib());
        format!(
            "{} • {} WIB",
            title.unwrap_or_else(|| "Absensi Gerbang".into()),
            t.format("%H:%M")
        )
    });

    let permits = permits?
        .into_iter()
        .map(|p| {
            let (status_label, status_kind) =
                permit_stage(&p.parent_status, &p.pamong_status, &p.guru_status, p.require_pamong);
            PermitItem {
                kind_label: permit_kind_label(&p.kind).into(),
                range_label: fmt_range(p.start_date, p.end_date),
                status_label: status_label.into(),
                status_kind: status_kind.into(),
            }
        })
        .collect();

    Ok(IzinData {
        pct,
        hadir,
        absen: alpa,
        points,
        detected,
        permits,
    })
}

/// Ajukan izin baru (validasi di sini — jangan percaya klien). `requested_by`
/// = santri sendiri ATAU orang tua yang mengajukan atas nama anak (lihat
/// `service::parent::submit_child_permit`) — menentukan `parent_status` awal
/// (migrasi 17): diajukan santri sendiri → 'pending' (perlu konfirmasi orang
/// tua); diajukan orang tua → 'approved' (parent adalah pengaju, konsen
/// tersirat).
#[allow(clippy::too_many_arguments)]
pub async fn submit_permit(
    pool: &Pool,
    user_id: i64,
    requested_by: i64,
    kind: &str,
    start: &str,
    end: &str,
    reason: &str,
) -> Result<()> {
    if !matches!(kind, "sick" | "leave" | "keperluan") {
        bail!("Jenis izin tidak valid.");
    }
    let Ok(start_date) = NaiveDate::parse_from_str(start.trim(), "%Y-%m-%d") else {
        bail!("Tanggal mulai wajib diisi.");
    };
    let end_date = match end.trim() {
        "" => None,
        s => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) if d >= start_date => Some(d),
            Ok(_) => bail!("Tanggal selesai tidak boleh sebelum tanggal mulai."),
            Err(_) => bail!("Tanggal selesai tidak valid."),
        },
    };
    let reason = reason.trim();
    if reason.chars().count() < 5 {
        bail!("Tuliskan alasan izin (minimal 5 karakter).");
    }
    let reason: String = reason.chars().take(500).collect();

    let parent_status = if requested_by == user_id { "pending" } else { "approved" };
    repo::insert_permit(
        pool, user_id, requested_by, kind, start_date, end_date, &reason, parent_status,
    )
    .await?;
    Ok(())
}

/// Data profil pengguna.
pub async fn profil(pool: &Pool, user_id: i64) -> Result<ProfilData> {
    let Some(p) = repo::profil_row(pool, user_id).await? else {
        bail!("unauth");
    };
    let role_label = match p.role.as_str() {
        "admin" => "ADMIN",
        "teacher" => "DEWAN GURU",
        "supervisor" => "PAMONG",
        "santri" => "SANTRI",
        "parent" => "ORANG TUA",
        _ => "PENGGUNA",
    };
    let ipk_history = repo::list_ipk(pool, user_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(id, semester, ipk)| crate::models::IpkItem { id, semester, ipk })
        .collect();
    Ok(ProfilData {
        name: p.full_name,
        username: p.username.unwrap_or_default(),
        role: p.role.clone(),
        role_label: role_label.into(),
        email: p.email,
        phone: p.phone_number,
        address: p.address,
        nis: p.nis,
        points: p.points,
        campus: p.campus,
        major: p.major,
        gender: p.gender,
        ipk_history,
    })
}

/// Ubah profil mahasiswa (kampus/jurusan/gender). Kosong → NULL.
pub async fn update_profile_extra(
    pool: &Pool,
    user_id: i64,
    campus: &str,
    major: &str,
    gender: &str,
) -> Result<()> {
    let opt = |s: &str| -> Option<String> {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    let g = match gender.trim() {
        "L" | "P" => Some(gender.trim().to_string()),
        _ => None,
    };
    repo::update_profile_extra(
        pool,
        user_id,
        opt(campus).as_deref(),
        opt(major).as_deref(),
        g.as_deref(),
    )
    .await
}

/// Tambah entri IPK santri. `ipk` teks (0.00–4.00).
pub async fn add_ipk(pool: &Pool, user_id: i64, semester: &str, ipk: &str) -> Result<i64> {
    let semester = semester.trim();
    if semester.is_empty() {
        bail!("Semester wajib diisi (mis. 2024/2025 Ganjil).");
    }
    let val: f64 = ipk
        .trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| anyhow::anyhow!("IPK harus berupa angka (mis. 3.75)."))?;
    if !(0.0..=4.0).contains(&val) {
        bail!("IPK harus di antara 0.00 sampai 4.00.");
    }
    repo::add_ipk(pool, user_id, semester, val).await
}

pub async fn delete_ipk(pool: &Pool, user_id: i64, id: i64) -> Result<()> {
    if !repo::delete_ipk(pool, user_id, id).await? {
        bail!("Entri IPK tidak ditemukan.");
    }
    Ok(())
}
