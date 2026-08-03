//! service/laporan.rs — Payload halaman /laporan, dibedakan per peran
//! (menggantikan item Profil di navbar). Guru/dewan guru REUSE `analisis_data`
//! (sudah lengkap: kehadiran, tren, ranking kelas) — di sini hanya menambah
//! ranking hafalan "Santri Teladan".

use anyhow::Result;
use chrono::Datelike;
use deadpool_postgres::Pool;

use super::fmt::fmt_dt_full;
use crate::models::{
    ClassPerfRow, HafalanRankItem, LaporanAdminData, LaporanGuruExtra, LaporanOrtuData,
    LaporanPointItem, LaporanSantriData, RecentPointRow, SessionUser, TrendBar,
};
use crate::repository as repo;

const HARI_PENDEK: [&str; 7] = ["Min", "Sen", "Sel", "Rab", "Kam", "Jum", "Sab"];

fn class_status(pct: i64) -> (&'static str, &'static str) {
    if pct >= 90 {
        ("Sangat Baik", "excellent")
    } else if pct >= 75 {
        ("Baik", "normal")
    } else {
        ("Perlu Perhatian", "attention")
    }
}

/// Laporan Institusi — admin & pamong (peran diperiksa di server fn caller).
pub async fn laporan_admin(pool: &Pool) -> Result<LaporanAdminData> {
    let (summary, trend, ranking, violation, points_recent, outside) = tokio::join!(
        repo::analisis_summary(pool, None),
        repo::attendance_trend_7d(pool, None),
        repo::class_ranking(pool, None, 8),
        repo::active_violation_points(pool),
        repo::recent_points_all(pool, 6),
        repo::count_outside(pool),
    );
    let (attendance_pct, _avg_points, _sessions_verified) = summary?;

    let trend = trend?
        .into_iter()
        .map(|(d, pct)| TrendBar {
            label: HARI_PENDEK[d.weekday().num_days_from_sunday() as usize].into(),
            pct: pct as i64,
        })
        .collect();

    let classes = ranking?
        .into_iter()
        .map(|r| {
            let (status_label, status_kind) = class_status(r.attendance_pct as i64);
            ClassPerfRow {
                name: r.name,
                member_count: r.santri_count,
                attendance_pct: r.attendance_pct as i64,
                status_label: status_label.into(),
                status_kind: status_kind.into(),
            }
        })
        .collect();

    let points_recent = points_recent?
        .into_iter()
        .map(|(name, class_name, reason, delta)| RecentPointRow { name, class_name, reason, delta })
        .collect();

    Ok(LaporanAdminData {
        attendance_pct: attendance_pct as i64,
        active_violation_points: violation?,
        santri_di_luar: outside?,
        trend,
        points_recent,
        classes,
    })
}

/// Ekstensi laporan guru/dewan guru: ranking hafalan (di atas `analisis_data`
/// yang sudah dipanggil terpisah oleh klien).
pub async fn laporan_guru_extra(pool: &Pool) -> Result<LaporanGuruExtra> {
    let rows = repo::top_hafalan(pool, 6).await?;
    Ok(LaporanGuruExtra {
        hafalan_top: rows
            .into_iter()
            .map(|(user_id, name, class_name, juz_count, points)| HafalanRankItem {
                user_id,
                name,
                class_name,
                juz_count,
                points,
            })
            .collect(),
    })
}

fn split_points(
    rows: Vec<(String, i32, chrono::DateTime<chrono::Utc>)>,
) -> (Vec<LaporanPointItem>, Vec<LaporanPointItem>) {
    let mut prestasi = Vec::new();
    let mut pelanggaran = Vec::new();
    for (reason, delta, at) in rows {
        let item = LaporanPointItem { reason, delta, date_label: fmt_dt_full(at) };
        if delta >= 0 {
            prestasi.push(item);
        } else {
            pelanggaran.push(item);
        }
    }
    (prestasi, pelanggaran)
}

/// Rapor Pribadi Santri.
pub async fn laporan_santri(pool: &Pool, user: &SessionUser) -> Result<LaporanSantriData> {
    let (since, _label) = super::santri::current_semester(pool).await?;
    // Prestasi & pelanggaran di rapor santri sengaja dipersempit ke 3 HARI
    // KALENDER WIB terakhir (hari ini, kemarin, kemarin lusa) — santri butuh
    // melihat yang baru saja terjadi, bukan daftar panjang sepanjang semester.
    // Statistik kehadiran & saldo poin di kartu atas tetap se-semester.
    let points_since = super::fmt::days_ago_wib(crate::models::RECENT_POINTS_DAYS - 1);
    let (stats, points, hafalan, juz, gate, home) = tokio::join!(
        repo::semester_stats(pool, user.id, since),
        repo::point_history_of(pool, user.id, 30, Some(points_since)),
        super::hafalan::hafalan_history(pool, user.id, 10),
        repo::juz_count(pool, user.id),
        repo::gate_status_of(pool, user.id),
        repo::user_home(pool, user.id),
    );
    let (hadir, izin, alpa, total) = stats?;
    let pct = if total > 0 { (hadir * 100) / total } else { 0 };
    let (prestasi, pelanggaran) = split_points(points?);
    let (gate_status, gate_at) = gate?;
    let points = home?.map(|h| h.points as i64).unwrap_or(0);

    Ok(LaporanSantriData {
        hadir,
        izin,
        alpa,
        attendance_pct: pct,
        points,
        prestasi,
        pelanggaran,
        hafalan: hafalan?,
        juz_count: juz?,
        gate_status,
        gate_at_label: gate_at.map(fmt_dt_full).unwrap_or_else(|| "-".into()),
    })
}

/// Laporan Santri untuk Orang Tua — `child_id` None → anak pertama terhubung.
pub async fn laporan_ortu(pool: &Pool, parent_id: i64, child_id: Option<i64>) -> Result<LaporanOrtuData> {
    let conns = repo::connections_of_parent(pool, parent_id).await?;
    let connected: Vec<i64> =
        conns.iter().filter(|c| c.status == "connected").map(|c| c.student_id).collect();
    let target = match child_id {
        Some(id) if connected.contains(&id) => id,
        Some(_) => bail_user!("forbidden"),
        None => *connected.first().ok_or_else(|| anyhow::anyhow!("Belum ada anak terhubung."))?,
    };

    let Some(info) = repo::child_info(pool, target).await? else {
        bail_user!("Santri tidak ditemukan.");
    };
    let (since, _label) = super::santri::current_semester(pool).await?;
    let (stats, points, hafalan, juz, gate, home) = tokio::join!(
        repo::semester_stats(pool, target, since),
        // Sama seperti rapor santri: 3 HARI KALENDER WIB terakhir. Orang tua
        // melihat hal yang sama dengan anaknya — kalau rentangnya beda, keduanya
        // bisa berdebat soal catatan yang sebenarnya sama.
        repo::point_history_of(pool, target, 30, Some(super::fmt::days_ago_wib(crate::models::RECENT_POINTS_DAYS - 1))),
        super::hafalan::hafalan_history(pool, target, 10),
        repo::juz_count(pool, target),
        repo::gate_status_of(pool, target),
        repo::user_home(pool, target),
    );
    let (hadir, izin, alpa, total) = stats?;
    let pct = if total > 0 { (hadir * 100) / total } else { 0 };
    let (prestasi, pelanggaran) = split_points(points?);
    let (gate_status, gate_at) = gate?;
    let points = home?.map(|h| h.points as i64).unwrap_or(0);

    Ok(LaporanOrtuData {
        child_id: target,
        child_name: info.full_name,
        class_name: info.class_name.unwrap_or_else(|| "-".into()),
        nis: info.nis.unwrap_or_else(|| "-".into()),
        hadir,
        izin,
        alpa,
        attendance_pct: pct,
        points,
        prestasi,
        pelanggaran,
        hafalan: hafalan?,
        juz_count: juz?,
        gate_status,
        gate_at_label: gate_at.map(fmt_dt_full).unwrap_or_else(|| "-".into()),
    })
}
