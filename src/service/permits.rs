//! service/permits.rs — Pengajuan & antrean izin.
//!
//! Migrasi 46 — izin PER-KELAS. Satu pengajuan dipecah jadi beberapa baris
//! `permit_requests`, satu untuk tiap WALI KELAS yang kelasnya dilewati selama
//! rentang izin (lihat `split_permit_per_wali`). Tiap baris jalan sendiri:
//!   * two_stage   → PAMONG kelas (tahap 1) → WALI KELAS (final).
//!   * direct_guru → WALI KELAS (final, pamong dilewati).
//! Mode default bisa dikonfigurasi admin (setelan `permit_approval_mode`), tapi
//! `require_pamong` per-kelas yang menang bila diset.
//!
//! Orang tua BUKAN penyetuju lagi — mereka hanya dinotifikasi & bisa melihat.

use std::collections::HashMap;

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

/// Kirim WA notifikasi izin baru ke penyetuju TIAP baris hasil pemecahan izin
/// (migrasi 46). Best-effort — gagal WA tak menggagalkan pengajuan.
///
/// Pesan menyebut kelas mana saja yang jadi tanggung jawab penerima, supaya
/// wali kelas langsung tahu konteksnya tanpa membuka aplikasi.
///   • Wali kelas (penyetuju final): SELALU diberi tahu.
///   • Pamong (tahap-1): hanya bila kelas itu verifikasi 2 langkah.
pub async fn notify_permit_splits(
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    pool: &Pool,
    student_id: i64,
    splits: &[PermitSplit],
    by_parent: bool,
) {
    let pemohon = if by_parent { "orang tua" } else { "santri sendiri" };
    for sp in splits {
        let t = match repo::permit_notify_targets(pool, student_id, sp.class_id).await {
            Ok(Some(t)) => t,
            _ => continue,
        };
        let kelas = if sp.class_names.is_empty() {
            String::new()
        } else {
            format!("\nKelas terdampak: {}", sp.class_names.join(", "))
        };
        let msg = format!(
            "🔔 *Pengajuan Izin Baru*\nSantri: {}\nDiajukan oleh: {}{}\n\nMohon segera diproses di aplikasi PPM AFM.",
            t.student_name, pemohon, kelas
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

    // Izin yang LOLOS TAHAP AKHIR diwujudkan jadi baris absensi 'permit'/'sick'.
    // Tanpa ini kolom "Izin" di rekap selalu 0 dan aturan poin izin PRD tak
    // pernah berjalan — santri berizin cuma "tak punya baris".
    //
    // Best-effort: kegagalan di sini TIDAK membatalkan persetujuan yang sudah
    // tercatat. Izin tetap sah; yang tertinggal hanya baris absensinya, dan itu
    // masih bisa ditandai manual oleh guru/pamong bertugas.
    if approve && !matches!(role, "supervisor") {
        match repo::materialize_permit_attendance(pool, permit_id).await {
            Ok(n) if n > 0 => tracing::info!(permit_id, "izin → {n} baris absensi"),
            Ok(_) => {}
            Err(e) => tracing::warn!(permit_id, "gagal mewujudkan absensi izin: {e}"),
        }
    }
    Ok(())
}

/// Pecah SATU pengajuan izin jadi beberapa baris `permit_requests` — satu untuk
/// tiap WALI KELAS yang kelasnya dilewati selama rentang izin (migrasi 46).
///
/// Contoh: izin 2 hari melewati kelas A (wali X), B & C (wali Y) → 2 baris:
/// satu ke wali X (kelas A), satu ke wali Y (kelas B & C digabung, karena
/// penyetujunya orang yang sama — tak perlu minta dua kali ke orang yang sama).
///
/// Bila santri tak punya kelas terjadwal di rentang itu, dibuat SATU baris
/// tanpa `class_id` (fallback ke kelas utama santri saat approval) supaya izin
/// tetap tercatat dan tak hilang diam-diam.
///
/// Return: `(permit_id, daftar nama kelas, nama wali)` per baris — dipakai
/// pemanggil untuk menyusun pesan notifikasi yang menyebut kelas mana saja.
pub async fn split_permit_per_wali(
    pool: &Pool,
    student_id: i64,
    requested_by: i64,
    kind: &str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
    reason: &str,
) -> Result<Vec<PermitSplit>> {
    // end_date None = izin sehari → rentang [start, start].
    let range_end = end_date.unwrap_or(start_date);
    let affected = repo::affected_classes(pool, student_id, start_date, range_end).await?;

    // Tak ada kelas terjadwal → satu baris tanpa class_id (fallback approval
    // memakai kelas utama santri, sama seperti perilaku sebelum migrasi 46).
    if affected.is_empty() {
        let id = repo::insert_permit(
            pool, student_id, requested_by, kind, start_date, end_date, reason, None, None,
        )
        .await?;
        return Ok(vec![PermitSplit {
            permit_id: id,
            class_id: None,
            class_names: Vec::new(),
            wali_name: None,
        }]);
    }

    // Kelompokkan per wali kelas. Kunci `Option<i64>` — kelas tanpa wali
    // (wali_kelas_id NULL) jadi satu grup sendiri yang diputus dewan guru/admin.
    let mut order: Vec<Option<i64>> = Vec::new();
    let mut groups: HashMap<Option<i64>, Vec<&repo::AffectedClass>> = HashMap::new();
    for c in &affected {
        let e = groups.entry(c.wali_kelas_id).or_default();
        if e.is_empty() {
            order.push(c.wali_kelas_id);
        }
        e.push(c);
    }

    let mut out = Vec::with_capacity(order.len());
    for wali_id in order {
        let classes = &groups[&wali_id];
        // class_id yang disimpan = kelas PERTAMA grup ini; dipakai approval untuk
        // menentukan require_pamong & pamong penanggung jawab.
        let first = classes[0];
        let permit_id = repo::insert_permit(
            pool,
            student_id,
            requested_by,
            kind,
            start_date,
            end_date,
            reason,
            Some(first.class_id),
            wali_id,
        )
        .await?;
        out.push(PermitSplit {
            permit_id,
            class_id: Some(first.class_id),
            class_names: classes.iter().map(|c| c.class_name.clone()).collect(),
            wali_name: first.wali_name.clone(),
        });
    }
    Ok(out)
}

/// Satu baris hasil pemecahan izin — dipakai untuk notifikasi & pesan ke santri.
pub struct PermitSplit {
    pub permit_id: i64,
    /// Kelas acuan approval (menentukan require_pamong & pamong penanggung
    /// jawab). None = santri tak punya kelas terjadwal di rentang izin.
    pub class_id: Option<i64>,
    /// Kelas-kelas yang jadi tanggung jawab wali ini selama rentang izin.
    pub class_names: Vec<String>,
    pub wali_name: Option<String>,
}
