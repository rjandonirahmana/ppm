//! service/parent.rs — Logika sisi ORANG TUA: cari santri, koneksi (approval
//! santri), pantauan banyak anak, izin & riwayat anak (dgn guard kepemilikan).

use anyhow::Result;
use chrono::Utc;
use deadpool_postgres::Pool;

use super::fmt::{fmt_ago, fmt_range, wib};
use crate::models::{
    permit_kind_label, permit_stage, ChildChip, ChildMonitor, ChildRiwayat, ConnRequest,
    ParentHome, ParentPermitItem, PendingConn, PermitItem,
    StudentSearchItem, TodayStatus,
};
use crate::repository as repo;

/// Cari santri (nama/NIS) untuk dikirimi permintaan koneksi.
pub async fn search_students(pool: &Pool, q: &str) -> Result<Vec<StudentSearchItem>> {
    let q = q.trim();
    if q.chars().count() < 2 {
        return Ok(Vec::new());
    }
    Ok(repo::search_students(pool, q, 8)
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

/// Kirim permintaan koneksi (menunggu persetujuan SANTRI).
pub async fn request_connection(pool: &Pool, parent_id: i64, student_id: i64) -> Result<()> {
    // Pastikan target memang santri.
    if repo::child_info(pool, student_id).await?.is_none() {
        bail_user!("Santri tidak ditemukan.");
    }
    if !repo::insert_connection(pool, parent_id, student_id).await? {
        bail_user!("Permintaan sudah pernah dikirim ke santri ini.");
    }
    Ok(())
}

fn status_today(status: &str) -> &'static str {
    match status {
        "late" => "Terlambat Masuk",
        "permit" | "sick" => "Izin",
        "absent" => "Tidak Hadir",
        _ => "Hadir",
    }
}

/// Panel pantauan satu anak (guard koneksi DILAKUKAN PEMANGGIL).
async fn monitor_child(pool: &Pool, child_id: i64) -> Result<Option<ChildMonitor>> {
    let Some(info) = repo::child_info(pool, child_id).await? else {
        return Ok(None);
    };
    let today = Utc::now().with_timezone(&wib()).date_naive();

    let (scan, counts, permits) = tokio::join!(
        repo::latest_scan_today(pool, child_id, today),
        repo::month_counts(pool, child_id),
        repo::list_my_permits(pool, child_id, 3),
    );

    // Status hari ini dari scan terakhir. Label mengikuti status ASLI absensi —
    // dulu dipatok "present", sehingga anak yang TERLAMBAT tetap terlihat tepat
    // waktu di mata orang tuanya.
    let today_status = scan?.map(|(title, ts, status)| {
        let t = ts.with_timezone(&wib());
        TodayStatus {
            label: status_today(&status).to_string(),
            time: format!("{} WIB", t.format("%H:%M")),
            gate: title.unwrap_or_else(|| "Gerbang Utama".into()),
        }
    });

    let (hadir, terlambat, absen) = counts?;
    let total = hadir + terlambat + absen;
    let pct = if total > 0 {
        (((hadir + terlambat) * 100) / total) as i32
    } else {
        0
    };

    let permits = permits?
        .into_iter()
        .map(|p| {
            let (status_label, status_kind) =
                permit_stage(&p.pamong_status, &p.guru_status, p.require_pamong);
            PermitItem {
                id: p.id,
                diajukan_oleh: if p.oleh_ortu {
                    format!("Diajukan orang tua — {}", p.requester_name)
                } else {
                    String::new()
                },
                kind_label: permit_kind_label(&p.kind).into(),
                range_label: fmt_range(p.start_date, p.end_date),
                class_label: p.class_name.unwrap_or_default(),
                status_label: status_label.into(),
                status_kind: status_kind.into(),
            }
        })
        .collect();

    Ok(Some(ChildMonitor {
        id: info.id,
        name: info.full_name,
        nis: info.nis.unwrap_or_else(|| "-".into()),
        class_name: info.class_name.unwrap_or_else(|| "-".into()),
        today: today_status,
        pct,
        hadir,
        terlambat,
        absen,
        permits,
    }))
}

/// Beranda orang tua: anak terhubung + permintaan menunggu + pantauan anak terpilih.
pub async fn parent_home(pool: &Pool, parent_id: i64, child: Option<i64>) -> Result<ParentHome> {
    let conns = repo::connections_of_parent(pool, parent_id).await?;

    let children: Vec<ChildChip> = conns
        .iter()
        .filter(|c| c.status == "connected")
        .map(|c| ChildChip {
            id: c.student_id,
            name: c.student_name.clone(),
        })
        .collect();

    let pending: Vec<PendingConn> = conns
        .iter()
        .filter(|c| c.status == "pending")
        .map(|c| PendingConn {
            id: c.id,
            student_name: c.student_name.clone(),
            since_label: format!("Terkirim: {}", fmt_ago(c.requested_at)),
        })
        .collect();

    // Anak terpilih: parameter (harus milik sendiri) atau anak pertama.
    let selected = match child {
        Some(id) if children.iter().any(|c| c.id == id) => Some(id),
        Some(_) => None, // bukan anaknya → abaikan (jangan bocorkan data)
        None => children.first().map(|c| c.id),
    };
    let monitor = match selected {
        Some(id) => monitor_child(pool, id).await?,
        None => None,
    };

    Ok(ParentHome {
        children,
        pending,
        monitor,
    })
}

/// Riwayat kehadiran ANAK (guard: harus terhubung).
pub async fn child_riwayat(pool: &Pool, parent_id: i64, child_id: i64) -> Result<ChildRiwayat> {
    if !repo::is_connected(pool, parent_id, child_id).await? {
        bail_user!("forbidden");
    }
    let Some(info) = repo::child_info(pool, child_id).await? else {
        bail_user!("Santri tidak ditemukan.");
    };
    let data = super::santri::riwayat(pool, child_id).await?;
    Ok(ChildRiwayat {
        child: ChildChip {
            id: info.id,
            name: info.full_name,
        },
        data,
    })
}

/// Daftar izin seluruh anak yang terhubung.
pub async fn children_permits(pool: &Pool, parent_id: i64) -> Result<Vec<ParentPermitItem>> {
    Ok(repo::permits_of_children(pool, parent_id, 20)
        .await?
        .into_iter()
        .map(|p| {
            let (status_label, status_kind) =
                permit_stage(&p.pamong_status, &p.guru_status, p.require_pamong);
            ParentPermitItem {
                id: p.id,
                diajukan_oleh: if p.oleh_ortu {
                    format!("Diajukan orang tua — {}", p.requester_name)
                } else {
                    "Diajukan santri sendiri".to_string()
                },
                child_name: p.child_name,
                kind_label: permit_kind_label(&p.kind).into(),
                range_label: fmt_range(p.start_date, p.end_date),
                reason: p.reason,
                status_label: status_label.into(),
                status_kind: status_kind.into(),
                when_label: fmt_ago(p.created_at),
            }
        })
        .collect())
}

/// Orang tua mengajukan izin UNTUK anaknya (guard: harus terhubung).
pub async fn submit_child_permit(
    pool: &Pool,
    parent_id: i64,
    child_id: i64,
    kind: &str,
    start: &str,
    end: &str,
    jam_mulai: &str,
    jam_selesai: &str,
    reason: &str,
) -> Result<Vec<super::permits::PermitSplit>> {
    if !repo::is_connected(pool, parent_id, child_id).await? {
        bail_user!("forbidden");
    }
    super::santri::submit_permit(
        pool, child_id, parent_id, kind, start, end, jam_mulai, jam_selesai, reason,
    )
    .await
}

// Migrasi 46: `confirm_child_permit` DIHAPUS — persetujuan izin kini murni
// akademik (pamong kelas → wali kelas). Orang tua tetap bisa MENGAJUKAN izin
// untuk anaknya (`submit_child_permit`) dan melihat statusnya.

// ── Sisi SANTRI: menyetujui/menolak permintaan koneksi ────────────────────────

pub async fn incoming_requests(pool: &Pool, student_id: i64) -> Result<Vec<ConnRequest>> {
    Ok(repo::pending_for_student(pool, student_id)
        .await?
        .into_iter()
        .map(|r| ConnRequest {
            id: r.id,
            parent_name: r.parent_name,
            since_label: fmt_ago(r.requested_at),
        })
        .collect())
}

pub async fn respond_request(
    pool: &Pool,
    student_id: i64,
    conn_id: i64,
    approve: bool,
) -> Result<bool> {
    repo::respond_connection(pool, conn_id, student_id, approve).await
}
