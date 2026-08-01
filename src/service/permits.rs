//! service/permits.rs — Antrean izin sisi staf. Alur global bisa dikonfigurasi
//! admin (setelan `permit_approval_mode`):
//!   * two_stage   → Orang Tua → PAMONG (tahap 1) → GURU (final).
//!   * direct_guru → Orang Tua → GURU (final, pamong dilewati).
//! Tahap konfirmasi orang tua ada di service::parent.

use anyhow::{bail, Result};
use deadpool_postgres::Pool;

use super::fmt::{fmt_range, fmt_when};
use crate::models::{permit_kind_label, PermitQueueData, PermitReviewItem};
use crate::repository as repo;

/// Kunci setelan mode persetujuan izin.
pub const PERMIT_MODE_KEY: &str = "permit_approval_mode";

/// Normalisasi HP untuk chat-ID WAHA (08.. → 62..).
fn wa_phone(p: &str) -> String {
    let d: String = p.chars().filter(|c| c.is_ascii_digit()).collect();
    match d.strip_prefix('0') {
        Some(rest) => format!("62{rest}"),
        None => d,
    }
}

/// Kirim WA notifikasi izin baru ke penyetuju kelas UTAMA santri. Best-effort
/// (gagal WA tak menggagalkan pengajuan). `by_parent` = pemohon orang tua.
///   • Wali kelas (penyetuju final): SELALU diberi tahu.
///   • Pamong (tahap-1): hanya bila kelas verifikasi 2 langkah (require_pamong).
pub async fn notify_permit(
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    pool: &Pool,
    student_id: i64,
    by_parent: bool,
) {
    let t = match repo::permit_notify_targets(pool, student_id).await {
        Ok(Some(t)) => t,
        _ => return,
    };
    let pemohon = if by_parent { "orang tua" } else { "santri sendiri" };
    let msg = format!(
        "🔔 *Pengajuan Izin Baru*\nSantri: {}\nDiajukan oleh: {}\n\nMohon segera diproses di aplikasi PPM AFM.",
        t.student_name, pemohon
    );
    if t.require_pamong {
        if let Some(phone) = t.pamong_phone.as_deref().filter(|p| !p.is_empty()) {
            let _ = super::registration::send_wa_text(http, waha, &wa_phone(phone), &msg).await;
        }
    }
    if let Some(phone) = t.wali_phone.as_deref().filter(|p| !p.is_empty()) {
        let _ = super::registration::send_wa_text(http, waha, &wa_phone(phone), &msg).await;
    }
}

/// Mode persetujuan izin saat ini ("two_stage" | "direct_guru"). Default
/// "two_stage" bila belum diset.
pub async fn approval_mode(pool: &Pool) -> String {
    repo::get_setting(pool, PERMIT_MODE_KEY)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "two_stage".to_string())
}

/// true bila mode = two_stage (pamong dulu, baru guru).
pub async fn is_two_stage(pool: &Pool) -> bool {
    approval_mode(pool).await == "two_stage"
}

/// Ubah mode persetujuan izin (admin). Validasi nilai.
pub async fn set_approval_mode(pool: &Pool, mode: &str) -> Result<()> {
    if !matches!(mode, "two_stage" | "direct_guru") {
        bail!("Mode persetujuan tidak valid.");
    }
    repo::set_setting(pool, PERMIT_MODE_KEY, mode).await
}

fn to_review_items(rows: Vec<repo::PendingPamongRow>) -> Vec<PermitReviewItem> {
    rows.into_iter()
        .map(|p| PermitReviewItem {
            id: p.id,
            student_name: p.student_name,
            nis: p.nis.unwrap_or_else(|| "-".into()),
            class_name: p.class_name.unwrap_or_else(|| "-".into()),
            kind_label: permit_kind_label(&p.kind).into(),
            range_label: fmt_range(p.start_date, p.end_date),
            reason: p.reason,
            when_label: fmt_when(p.created_at),
        })
        .collect()
}

/// Payload /izin-staf disesuaikan PERAN peninjau (rute PER-KELAS, migrasi 29):
/// - supervisor (pamong) → izin kelas yang WAJIB via pamong (require_pamong);
/// - teacher (wali kelas) → izin santri KELAS-nya (wali_kelas_id = dia);
/// - dewan_guru/admin → SEMUA izin tahap final (oversight).
/// `default_require` (mode global /setelan) = fallback santri tanpa kelas utama.
pub async fn permit_queue(pool: &Pool, role: &str, user_id: i64) -> Result<PermitQueueData> {
    let default_require = is_two_stage(pool).await;

    if role == "supervisor" {
        // Pamong hanya lihat izin santri KELAS yang ia ampu (pamong_id = dia).
        let pamong_id = Some(user_id);
        let (pending, decided_today) = tokio::join!(
            repo::pending_pamong_permits(pool, default_require, pamong_id, 50),
            repo::pamong_permits_decided_today(pool, pamong_id),
        );
        let items = to_review_items(pending?);
        return Ok(PermitQueueData {
            pending_count: items.len() as i64,
            approved_today: decided_today?,
            items,
            two_stage: default_require,
            stage_label: "Persetujuan Pamong".into(),
        });
    }

    // Wali kelas (teacher) → hanya kelasnya; admin → semua (superuser).
    // Dewan guru tak lagi diberi akses izin santri (gate di api.rs).
    let wali_id = (role == "teacher").then_some(user_id);
    let (pending, decided_today) = tokio::join!(
        repo::pending_guru_permits(pool, wali_id, default_require, 50),
        repo::guru_permits_decided_today(pool, wali_id),
    );
    let items = to_review_items(pending?);
    Ok(PermitQueueData {
        pending_count: items.len() as i64,
        approved_today: decided_today?,
        items,
        two_stage: default_require,
        stage_label: "Persetujuan Wali Kelas".into(),
    })
}

/// Setujui/tolak izin. Rute sesuai peran: pamong → tahap 1 (hanya kelas
/// require_pamong); teacher → keputusan final HANYA izin kelasnya; dewan
/// guru/admin → keputusan final izin mana pun.
pub async fn decide_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    role: &str,
    staff_id: i64,
) -> Result<()> {
    let default_require = is_two_stage(pool).await;
    let ok = if role == "supervisor" {
        repo::decide_pamong_permit(pool, permit_id, approve, default_require, Some(staff_id), staff_id)
            .await?
    } else {
        let wali_id = (role == "teacher").then_some(staff_id);
        repo::decide_guru_permit(pool, permit_id, approve, wali_id, default_require, staff_id).await?
    };
    if !ok {
        bail!("Izin tidak ditemukan, sudah diproses, atau di luar wewenang Anda.");
    }
    Ok(())
}

/// Auto-create permit requests per wali kelas unik yang affected.
///
/// Logika:
/// 1. Query class_schedules yang overlap dengan izin periode (start_date..end_date)
/// 2. Group by wali_kelas_id → kumpulkan kelas unique per guru/wali
/// 3. Buat permit_request per wali_kelas_id dengan link class_id (first affected class)
/// 4. Setiap permit perlu approval dari:
///    - Pamong kelas (jika require_pamong)
///    - Wali kelas bersangkutan (final)
///
/// Return: list of created permit_ids + detail (class_name, wali_name) untuk notifikasi
pub async fn auto_create_permits_per_wali(
    pool: &Pool,
    student_id: i64,
    requested_by: i64,
    permit_kind: &str,
    start_date: chrono::NaiveDate,
    end_date: chrono::NaiveDate,
    reason: &str,
) -> Result<Vec<(i64, String, String)>> {
    use chrono::NaiveDate;

    let c = pool.get().await?;

    // Query: kelas yang affected dalam rentang izin, grouped by wali_kelas_id unik
    let rows = c
        .query(
            "SELECT DISTINCT \
                    c.id, c.name, c.wali_kelas_id, u.full_name, c.require_pamong \
             FROM class_schedules cs \
             JOIN classes c ON c.id = cs.class_id \
             LEFT JOIN users u ON u.id = c.wali_kelas_id \
             JOIN class_participants cp ON cp.class_schedule_id = cs.id \
             WHERE cp.user_id = $1 \
                AND cs.status = 'active' \
                AND cs.start_date <= $3 \
                AND cs.end_date >= $2 \
             ORDER BY c.wali_kelas_id, c.name",
            &[&student_id, &start_date, &end_date],
        )
        .await?;

    let mut created = Vec::new();
    let mut last_wali_id: Option<i64> = None;

    for row in rows {
        let class_id: i64 = row.get(0);
        let class_name: String = row.get(1);
        let wali_id: Option<i64> = row.get(2);
        let wali_name: String = row.get(3);
        let _require_pamong: bool = row.get(4);

        // Skip jika sudah ada permit untuk wali_id ini (group by wali, buat 1 per wali saja)
        if last_wali_id == wali_id {
            continue;
        }
        last_wali_id = wali_id;

        // Create permit_request untuk wali_id ini
        let permit_row = c
            .query_one(
                "INSERT INTO permit_requests \
                    (user_id, requested_by, type, reason, start_date, end_date, class_id, wali_kelas_id, \
                     pamong_status, guru_status) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', 'pending') \
                 RETURNING id",
                &[
                    &student_id,
                    &requested_by,
                    &permit_kind,
                    &reason,
                    &start_date,
                    &end_date,
                    &class_id,
                    &wali_id,
                ],
            )
            .await?;

        let permit_id: i64 = permit_row.get(0);
        created.push((permit_id, class_name, wali_name));
    }

    Ok(created)
}
