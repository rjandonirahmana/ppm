//! repository/permits.rs — Query tabel permit_requests (izin/sakit/keperluan).
//!
//! Migrasi 46 — izin PER-KELAS. Satu pengajuan izin memecah diri jadi beberapa
//! baris `permit_requests`: satu untuk tiap WALI KELAS yang kelasnya dilewati
//! selama rentang izin. Contoh: izin 2 hari melewati kelas A (wali X), B & C
//! (wali Y) → 2 baris, satu ke wali X satu ke wali Y.
//!
//! Alur per-baris: Pamong kelas (bila `require_pamong` DAN kelas itu punya
//! pamong) → Wali Kelas (FINAL). Kelas ber-`require_pamong` tapi `pamong_id`
//! masih NULL TIDAK memblokir izin — tahap pamong dilewati, izin langsung ke
//! wali kelas. Tanpa ini izin macet permanen: tak ada pamong yang cocok untuk
//! menyetujui, sementara tahap final menuntut `pamong_status = 'approved'`.
//! Orang tua TIDAK lagi jadi penyetuju (kolom parent_* dihapus di migrasi 46) —
//! izin adalah urusan akademik; orang tua cukup dinotifikasi.

use anyhow::{Context, Result};
use chrono::NaiveDate;
use deadpool_postgres::Pool;

/// Target notifikasi izin: penyetuju kelas UTAMA santri (wali kelas selalu;
/// pamong hanya bila `require_pamong` = verifikasi 2 langkah).
pub struct PermitNotifyTargets {
    pub student_name: String,
    pub require_pamong: bool,
    pub wali_name: Option<String>,
    pub wali_phone: Option<String>,
    pub pamong_name: Option<String>,
    pub pamong_phone: Option<String>,
}

/// Ambil wali kelas + pamong (nama & HP) penyetuju izin.
///
/// `class_id` Some = kelas TUJUAN izin (migrasi 46) — dipakai saat satu ajuan
/// terpecah ke beberapa wali; None = fallback ke kelas UTAMA (is_primary)
/// santri, untuk izin lama atau santri tanpa kelas terjadwal.
pub async fn permit_notify_targets(
    pool: &Pool,
    student_id: i64,
    class_id: Option<i64>,
) -> Result<Option<PermitNotifyTargets>> {
    let c = pool.get().await?;
    let row = c
        .query_opt(
            "SELECT s.full_name, \
                    COALESCE(cl.require_pamong, TRUE), \
                    w.full_name, w.phone_number, \
                    pm.full_name, pm.phone_number \
             FROM users s \
             LEFT JOIN class_participants cp \
                 ON cp.user_id = s.id AND cp.is_primary AND $2::bigint IS NULL \
             LEFT JOIN classes cl ON cl.id = COALESCE($2::bigint, cp.class_id) \
             LEFT JOIN users w  ON w.id = cl.wali_kelas_id \
             LEFT JOIN users pm ON pm.id = cl.pamong_id \
             WHERE s.id = $1 \
             LIMIT 1",
            &[&student_id, &class_id],
        )
        .await
        .context("permit_notify_targets")?;
    Ok(row.map(|r| PermitNotifyTargets {
        student_name: r.get(0),
        require_pamong: r.get(1),
        wali_name: r.get(2),
        wali_phone: r.get(3),
        pamong_name: r.get(4),
        pamong_phone: r.get(5),
    }))
}

/// Satu kelas yang dilewati selama rentang izin, beserta penanggung jawabnya.
/// Dipakai `service::permits::auto_create_permits_per_wali` untuk memecah satu
/// pengajuan jadi beberapa `permit_requests` (satu per wali kelas unik).
pub struct AffectedClass {
    pub class_id: i64,
    pub class_name: String,
    pub wali_kelas_id: Option<i64>,
    pub wali_name: Option<String>,
    pub require_pamong: bool,
}

/// Kelas yang jadwalnya BERSINGGUNGAN dengan rentang izin [start, end] dan
/// santri terdaftar sebagai peserta. Terurut per wali kelas agar pemanggil bisa
/// mengelompokkan tanpa sorting ulang.
///
/// Catatan: memakai `class_schedules` (jadwal periodik) — sesi yang sudah
/// dimaterialisasi (`class_sessions`) tak dipakai karena izin bisa diajukan
/// untuk tanggal yang sesinya belum di-generate.
pub async fn affected_classes(
    pool: &Pool,
    student_id: i64,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<Vec<AffectedClass>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT DISTINCT cl.id, cl.name, cl.wali_kelas_id, w.full_name, \
                    COALESCE(cl.require_pamong, TRUE) \
             FROM class_schedules cs \
             JOIN classes cl ON cl.id = cs.class_id \
             LEFT JOIN users w ON w.id = cl.wali_kelas_id \
             JOIN class_participants cp \
                 ON (cp.class_schedule_id = cs.id OR cp.class_id = cl.id) \
                AND cp.user_id = $1 \
             WHERE cs.status = 'active' \
               AND COALESCE(cs.start_date, $2) <= $3 \
               AND COALESCE(cs.end_date, $3) >= $2 \
             ORDER BY cl.wali_kelas_id NULLS LAST, cl.name",
            &[&student_id, &start_date, &end_date],
        )
        .await
        .context("affected_classes")?;
    Ok(rows
        .into_iter()
        .map(|r| AffectedClass {
            class_id: r.get(0),
            class_name: r.get(1),
            wali_kelas_id: r.get(2),
            wali_name: r.get(3),
            require_pamong: r.get(4),
        })
        .collect())
}

/// Sisipkan SATU baris izin yang ditujukan ke satu kelas + wali kelas tertentu.
/// Dipanggil berulang oleh `auto_create_permits_per_wali` (sekali per wali unik).
#[allow(clippy::too_many_arguments)]
pub async fn insert_permit(
    pool: &Pool,
    user_id: i64,
    requested_by: i64,
    kind: &str,
    start_date: NaiveDate,
    end_date: Option<NaiveDate>,
    reason: &str,
    class_id: Option<i64>,
    wali_kelas_id: Option<i64>,
) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "INSERT INTO permit_requests \
                (user_id, requested_by, type, reason, start_date, end_date, class_id, wali_kelas_id) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) RETURNING id",
            &[
                &user_id,
                &requested_by,
                &kind,
                &reason,
                &start_date,
                &end_date,
                &class_id,
                &wali_kelas_id,
            ],
        )
        .await
        .context("insert_permit")?;
    Ok(row.get(0))
}

pub struct PermitRow {
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub pamong_status: String,
    pub guru_status: String,
    /// Rute lewat pamong? Diturunkan dari KELAS tujuan izin (migrasi 46),
    /// fallback ke kelas utama santri untuk baris lama hasil backfill.
    pub require_pamong: bool,
    /// Nama kelas yang izin ini tujukan — supaya santri tahu izinnya terpecah
    /// ke mana saja ("Menunggu Wali Kelas — Fiqih Lanjutan").
    pub class_name: Option<String>,
}

pub async fn list_my_permits(pool: &Pool, user_id: i64, limit: i64) -> Result<Vec<PermitRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT p.type, p.start_date, p.end_date, p.pamong_status, p.guru_status, \
                    COALESCE(tc.require_pamong, cl.require_pamong, TRUE), tc.name \
             FROM permit_requests p \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             LEFT JOIN class_participants cp ON cp.user_id = p.user_id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE p.user_id = $1 ORDER BY p.created_at DESC LIMIT $2",
            &[&user_id, &limit],
        )
        .await
        .context("list_my_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PermitRow {
            kind: r.get(0),
            start_date: r.get(1),
            end_date: r.get(2),
            pamong_status: r.get(3),
            guru_status: r.get(4),
            require_pamong: r.get(5),
            class_name: r.get(6),
        })
        .collect())
}

// CATATAN (migrasi 46): tahap konfirmasi ORANG TUA DIHAPUS. Fungsi
// `pending_parent_confirms` & `confirm_parent_permit` ikut dihapus — orang tua
// kini hanya MELIHAT izin anaknya (lihat repository/parents.rs), tak memutus.

// ── Tahap 1: PAMONG kelas ─────────────────────────────────────────────────────

pub struct PendingPamongRow {
    pub id: i64,
    pub student_name: String,
    pub nis: Option<String>,
    pub class_name: Option<String>,
    pub kind: String,
    pub start_date: NaiveDate,
    pub end_date: Option<NaiveDate>,
    pub reason: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Antrean pamong: izin yang menuju KELAS yang wajib via pamong
/// (`require_pamong`), menunggu keputusan pamong kelas itu. Guard `pamong_id`
/// memakai pamong KELAS TUJUAN izin (`p.class_id`), bukan kelas utama santri —
/// inilah inti migrasi 46: izin ke kelas X hanya boleh diputus pamong kelas X.
/// `default_require` = fallback bila izin lama belum punya `class_id`.
pub async fn pending_pamong_permits(
    pool: &Pool,
    default_require: bool,
    pamong_id: Option<i64>,
    limit: i64,
) -> Result<Vec<PendingPamongRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT p.id, u.full_name, u.nis, COALESCE(tc.name, cl.name), \
                p.type, p.start_date, p.end_date, p.reason, p.created_at \
             FROM permit_requests p JOIN users u ON u.id = p.user_id \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             LEFT JOIN class_participants cp ON cp.user_id = p.user_id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE p.pamong_status = 'pending' AND p.guru_status = 'pending' \
                AND COALESCE(tc.require_pamong, cl.require_pamong, $2) = TRUE \
                AND ($3::bigint IS NULL OR COALESCE(tc.pamong_id, cl.pamong_id) = $3) \
             ORDER BY p.created_at ASC LIMIT $1",
            &[&limit, &default_require, &pamong_id],
        )
        .await
        .context("pending_pamong_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingPamongRow {
            id: r.get(0),
            student_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kind: r.get(4),
            start_date: r.get(5),
            end_date: r.get(6),
            reason: r.get(7),
            created_at: r.get(8),
        })
        .collect())
}

/// Jumlah izin diputuskan pamong HARI INI (statistik antrean).
pub async fn pamong_permits_decided_today(pool: &Pool, pamong_id: Option<i64>) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FROM permit_requests p \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             LEFT JOIN class_participants cp ON cp.user_id = p.user_id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE p.pamong_status <> 'pending' \
                AND (p.pamong_at AT TIME ZONE 'Asia/Jakarta')::date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date \
                AND ($1::bigint IS NULL OR COALESCE(tc.pamong_id, cl.pamong_id) = $1)",
            &[&pamong_id],
        )
        .await
        .context("pamong_permits_decided_today")?;
    Ok(row.get(0))
}

/// Setujui/tolak izin oleh pamong (tahap 1). Guard: KELAS TUJUAN izin
/// (`p.class_id`) wajib via pamong (`require_pamong`) DAN pamong kelas itu =
/// `pamong_id`. Izin lama tanpa `class_id` jatuh ke kelas utama santri.
pub async fn decide_pamong_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    default_require: bool,
    pamong_id: Option<i64>,
    staff_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p SET pamong_status = $2, pamong_by = $3, pamong_at = NOW() \
             WHERE p.id = $1 AND p.pamong_status = 'pending' AND p.guru_status = 'pending' \
                AND COALESCE( \
                    (SELECT c.require_pamong FROM classes c WHERE c.id = p.class_id), \
                    (SELECT c.require_pamong FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id AND cp.is_primary LIMIT 1), $4) = TRUE \
                AND ($5::bigint IS NULL OR COALESCE( \
                    (SELECT c.pamong_id FROM classes c WHERE c.id = p.class_id), \
                    (SELECT c.pamong_id FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id AND cp.is_primary LIMIT 1)) = $5)",
            &[&permit_id, &status, &staff_id, &default_require, &pamong_id],
        )
        .await
        .context("decide_pamong_permit")?;
    Ok(n > 0)
}

// ── Tahap FINAL: WALI KELAS (guru penyetuju akhir) ────────────────────────────

/// Antrean wali kelas: izin yang DITUJUKAN ke guru ini (`p.wali_kelas_id`).
/// Prasyarat: bila kelas tujuan `require_pamong`, pamong harus sudah approve;
/// bila tidak, izin langsung masuk antrean wali kelas.
///
/// `wali_id` Some = hanya izin milik guru ini; None = semua (dewan guru/admin
/// oversight). `default_require` = fallback bila izin lama tak punya `class_id`.
pub async fn pending_guru_permits(
    pool: &Pool,
    wali_id: Option<i64>,
    default_require: bool,
    limit: i64,
) -> Result<Vec<PendingPamongRow>> {
    let c = pool.get().await?;
    let rows = c
        .query(
            "SELECT p.id, u.full_name, u.nis, COALESCE(tc.name, cl.name), \
                p.type, p.start_date, p.end_date, p.reason, p.created_at \
             FROM permit_requests p JOIN users u ON u.id = p.user_id \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             LEFT JOIN class_participants cp ON cp.user_id = p.user_id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE p.guru_status = 'pending' \
                AND p.pamong_status <> 'rejected' \
                AND CASE WHEN COALESCE(tc.require_pamong, cl.require_pamong, $2) \
                              AND COALESCE(tc.pamong_id, cl.pamong_id) IS NOT NULL \
                         THEN p.pamong_status = 'approved' \
                         ELSE TRUE END \
                AND ($3::bigint IS NULL \
                     OR COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id) = $3) \
             ORDER BY p.created_at ASC LIMIT $1",
            &[&limit, &default_require, &wali_id],
        )
        .await
        .context("pending_guru_permits")?;
    Ok(rows
        .into_iter()
        .map(|r| PendingPamongRow {
            id: r.get(0),
            student_name: r.get(1),
            nis: r.get(2),
            class_name: r.get(3),
            kind: r.get(4),
            start_date: r.get(5),
            end_date: r.get(6),
            reason: r.get(7),
            created_at: r.get(8),
        })
        .collect())
}

/// Jumlah izin diputuskan wali kelas (final) HARI INI. `wali_id` Some = hanya
/// keputusan atas izin kelas guru ini.
pub async fn guru_permits_decided_today(pool: &Pool, wali_id: Option<i64>) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "SELECT COUNT(*) FROM permit_requests p \
             LEFT JOIN classes tc ON tc.id = p.class_id \
             LEFT JOIN class_participants cp ON cp.user_id = p.user_id AND cp.is_primary \
             LEFT JOIN classes cl ON cl.id = cp.class_id \
             WHERE p.guru_status <> 'pending' \
                AND (p.guru_at AT TIME ZONE 'Asia/Jakarta')::date = (NOW() AT TIME ZONE 'Asia/Jakarta')::date \
                AND ($1::bigint IS NULL \
                     OR COALESCE(p.wali_kelas_id, tc.wali_kelas_id, cl.wali_kelas_id) = $1)",
            &[&wali_id],
        )
        .await
        .context("guru_permits_decided_today")?;
    Ok(row.get(0))
}

/// Keputusan FINAL wali kelas atas SATU baris izin (satu kelas tujuan).
/// Guard: bila kelas tujuan `require_pamong` maka pamong wajib sudah approve;
/// `guru_status` masih pending; dan (wali_id None ATAU wali kelas tujuan =
/// wali_id). Izin lama tanpa `class_id` jatuh ke kelas utama santri.
pub async fn decide_guru_permit(
    pool: &Pool,
    permit_id: i64,
    approve: bool,
    wali_id: Option<i64>,
    default_require: bool,
    staff_id: i64,
) -> Result<bool> {
    let status = if approve { "approved" } else { "rejected" };
    let c = pool.get().await?;
    let n = c
        .execute(
            "UPDATE permit_requests p SET guru_status = $2, guru_by = $3, guru_at = NOW() \
             WHERE p.id = $1 AND p.guru_status = 'pending' \
                AND p.pamong_status <> 'rejected' \
                AND CASE WHEN COALESCE( \
                        (SELECT c.require_pamong FROM classes c WHERE c.id = p.class_id), \
                        (SELECT c.require_pamong FROM class_participants cp \
                            JOIN classes c ON c.id = cp.class_id \
                            WHERE cp.user_id = p.user_id AND cp.is_primary LIMIT 1), $5) \
                          AND COALESCE( \
                        (SELECT c.pamong_id FROM classes c WHERE c.id = p.class_id), \
                        (SELECT c.pamong_id FROM class_participants cp \
                            JOIN classes c ON c.id = cp.class_id \
                            WHERE cp.user_id = p.user_id AND cp.is_primary LIMIT 1)) IS NOT NULL \
                     THEN p.pamong_status = 'approved' \
                     ELSE TRUE END \
                AND ($4::bigint IS NULL OR COALESCE( \
                     p.wali_kelas_id, \
                     (SELECT c.wali_kelas_id FROM classes c WHERE c.id = p.class_id), \
                     (SELECT c.wali_kelas_id FROM class_participants cp \
                        JOIN classes c ON c.id = cp.class_id \
                        WHERE cp.user_id = p.user_id AND cp.is_primary LIMIT 1)) = $4)",
            &[&permit_id, &status, &staff_id, &wali_id, &default_require],
        )
        .await
        .context("decide_guru_permit")?;
    Ok(n > 0)
}

/// Tuliskan baris absensi untuk izin yang SUDAH disetujui final.
///
/// MASALAH yang diselesaikan: sampai sekarang tak ada satu pun kode yang
/// menulis `attendances.status = 'permit'/'sick'`. Akibatnya kolom "Izin" di
/// rekap mingguan selalu 0, dan aturan PRD "izin mengurangi poin"
/// (`izin_points` migrasi 28, `attendance_delta("permit")`) tak pernah
/// berjalan — santri berizin sekadar TIDAK PUNYA baris, hanya dilewati
/// auto-absent.
///
/// Status yang ditulis mengikuti JENIS izin:
///   • `sick`  → status 'sick'  → 0 poin (PRD: sakit dgn surat sah tak memotong)
///   • lainnya → status 'permit' → −izin_points
///
/// `ON CONFLICT DO NOTHING`: baris yang SUDAH ada tak ditimpa. Santri yang
/// ternyata hadir sebagian, atau yang sudah terlanjur dialpakan auto-absent,
/// dibiarkan apa adanya — mengubahnya urusan koreksi manual oleh guru/pamong
/// bertugas (migrasi 51), bukan efek samping diam-diam dari persetujuan izin.
///
/// Verifikasi langsung 'approved': yang menyetujui izin adalah wali kelas, dan
/// dialah juga penyetuju akhir absensi. Melewatkannya ke antrean berarti
/// memintanya menyetujui hal yang sama dua kali.
///
/// Return jumlah baris absensi baru.
pub async fn materialize_permit_attendance(pool: &Pool, permit_id: i64) -> Result<i64> {
    let c = pool.get().await?;
    let row = c
        .query_one(
            "WITH p AS ( \
                SELECT pr.id, pr.user_id, pr.class_id, pr.type, \
                       pr.start_date, COALESCE(pr.end_date, pr.start_date) AS end_date, \
                       CASE WHEN pr.type = 'sick' THEN 'sick' ELSE 'permit' END AS att_status \
                  FROM permit_requests pr \
                 WHERE pr.id = $1 AND pr.guru_status = 'approved' \
             ), \
             ins AS ( \
                INSERT INTO attendances \
                    (user_id, class_session_id, class_schedule_id, status, method, \
                     pamong_status, pamong_at, verify_status, verified_at, \
                     note, gate_label, scanned_at, scan_date) \
                SELECT p.user_id, s.id, s.class_schedule_id, p.att_status, 'manual', \
                       'approved', NOW(), 'approved', NOW(), \
                       'Izin disetujui', 'system', NOW(), s.session_date \
                  FROM p \
                  JOIN class_sessions s ON s.class_id = p.class_id \
                   AND s.session_date BETWEEN p.start_date AND p.end_date \
                   AND s.status <> 'cancelled' \
                 ON CONFLICT (user_id, class_session_id) DO NOTHING \
                RETURNING id, user_id, class_schedule_id, status \
             ), \
             lg AS ( \
                INSERT INTO point_logs (user_id, delta, reason, category, attendance_id) \
                SELECT ins.user_id, \
                       -COALESCE(sch.izin_points, \
                                 cat_default_points(COALESCE(sch.activity_type,'other'),'izin'))::int, \
                       'Kehadiran (' || ins.status || ') — izin disetujui', 'discipline', ins.id \
                  FROM ins \
                  LEFT JOIN class_schedules sch ON sch.id = ins.class_schedule_id \
                 WHERE ins.status = 'permit' \
                RETURNING user_id \
             ) \
             SELECT COUNT(*)::bigint FROM ins",
            &[&permit_id],
        )
        .await
        .context("materialize_permit_attendance")?;
    Ok(row.get(0))
}
