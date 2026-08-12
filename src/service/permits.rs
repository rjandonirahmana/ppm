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


use anyhow::Result;
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
            "🔔 *Pengajuan Izin Baru*\nSantri: {}\nDiajukan oleh: {}{}\n\nMohon segera diproses di aplikasi AFM SMART.",
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
        bail_user!("Mode persetujuan tidak valid.");
    }
    repo::set_setting(pool, PERMIT_MODE_KEY, mode).await
}

/// Baris antrean + DAMPAKNYA: kelas apa saja yang terlewat bila disetujui.
///
/// Wali kelas memutuskan sambil melihat akibatnya. Rentang tanggal saja tak
/// menjawab "berapa kelas yang ia tinggalkan" — dan izin dua hari bisa berarti
/// dua sesi atau sepuluh, tergantung jadwalnya.
///
/// Dampaknya dihitung dari `permit_request_classes` (migrasi 64) — cakupan yang
/// SUDAH tersimpan saat izin dibuat, bukan dihitung ulang. Menghitung ulang di
/// sini bisa memberi jawaban berbeda bila jadwalnya berubah setelah pengajuan,
/// dan yang wajib dilihat wali adalah izin yang ia putuskan.
async fn to_review_items(pool: &Pool, rows: Vec<repo::PendingPamongRow>) -> Vec<PermitReviewItem> {
    let ids: Vec<i64> = rows.iter().map(|p| p.id).collect();
    let dampak = repo::dampak_izin(pool, &ids).await.unwrap_or_default();
    rows.into_iter()
        .map(|p| {
            let d = dampak.get(&p.id);
            PermitReviewItem {
                id: p.id,
                student_name: p.student_name,
                nis: p.nis.unwrap_or_else(|| "-".into()),
                class_name: p.class_name.unwrap_or_else(|| "-".into()),
                kind_label: permit_kind_label(&p.kind).into(),
                range_label: fmt_range(p.start_date, p.end_date),
                jam_label: match (p.start_time, p.end_time) {
                    (Some(a), Some(b)) => {
                        format!("{} – {} WIB", a.format("%H:%M"), b.format("%H:%M"))
                    }
                    _ => String::new(),
                },
                dua_tahap: p.dua_tahap,
                pamong_ok: p.pamong_ok,
                sesi_terlewat: d.map(|v| v.0.clone()).unwrap_or_default(),
                total_sesi: d.map(|v| v.1).unwrap_or(0),
                reason: p.reason,
                when_label: fmt_when(p.created_at),
            }
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
        let items = to_review_items(pool, pending?).await;
        return Ok(PermitQueueData {
            pending_count: items.len() as i64,
            approved_today: decided_today?,
            items,
            two_stage: default_require,
            stage_label: "Persetujuan Pamong".into(),
        });
    }

    // Guru hanya melihat izin santri di KELAS YANG IA AMPU — bukan semua.
    //
    // Dulu `wali_id` hanya diisi untuk role "teacher"; padahal peran itu sudah
    // digabung ke "dewan_guru" (migrasi 36), jadi praktis SELALU None dan
    // setiap guru melihat antrean seluruh pesantren. Admin pun sengaja tak
    // sampai ke sini lagi (gate di api.rs).
    let wali_id = Some(user_id);
    let (pending, decided_today) = tokio::join!(
        repo::pending_guru_permits(pool, wali_id, default_require, 50),
        repo::guru_permits_decided_today(pool, wali_id),
    );
    let items = to_review_items(pool, pending?).await;
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
pub async fn decide_permit(pool: &Pool, permit_id: i64, approve: bool, staff_id: i64) -> Result<()> {
    // DUA TAHAP, mengikuti setelan kelas KBM santri — bukan satu tahap saja.
    //
    // Sebelumnya fungsi ini langsung memanggil jalur wali kelas, dengan alasan
    // "tahap pamong untuk izin sudah dihapus". Tapi antreannya TIDAK ikut
    // dihapus: `pending_pamong_permits` masih memunculkan izin kepada pamong
    // untuk kelas ber-`require_pamong`. Jadi pamong melihat izin di layarnya,
    // menekan Setujui, dan request-nya masuk ke jalur wali — di sana ia bukan
    // wali siapa pun, sehingga selalu ditolak "Izin ini bukan milik kelas
    // Anda". Antreannya bekerja, tombolnya tidak.
    //
    // `decide_pamong_permit` sudah ada dan penjaganya cocok PERSIS dengan
    // penyaring antrean itu; ia cuma tak pernah dipanggil dari mana pun.
    //
    // Urutannya: coba tahap pamong dulu. Penjaga di dalamnya hanya cocok bila
    // SEMUA benar — pemutusnya memang pamong kelas acuan, kelasnya memang
    // menuntut dua tahap, dan izinnya memang masih di tahap itu. Kalau tidak
    // cocok, tak ada baris tersentuh dan keputusannya jatuh ke jalur wali.
    // Dengan begitu satu tombol melayani kedua peran tanpa layarnya perlu tahu
    // sedang berada di tahap mana.
    if repo::decide_pamong_permit(pool, permit_id, approve, false, Some(staff_id), staff_id).await?
    {
        // Tahap pamong saja: absensinya BELUM diwujudkan. Yang mengesahkan izin
        // tetap wali kelas di tahap berikutnya.
        return Ok(());
    }

    // Tahap FINAL: wali kelas KBM santri. Bukan dewan guru, bukan admin — izin
    // adalah urusan antara santri dan walinya, dan orang yang tak mengenal
    // santrinya tak punya dasar untuk menimbang. `wali_id = staf ini`, jadi
    // izin milik wali LAIN tak akan tersentuh.
    let ok = repo::decide_guru_permit(pool, permit_id, approve, Some(staff_id), false, staff_id)
        .await?;
    if !ok {
        bail_user!(
            "Izin ini bukan milik kelas Anda, atau sudah diproses. Yang boleh memutuskan \
             hanya pamong kelas KBM santri (bila kelasnya dua tahap) dan wali kelasnya."
        );
    }

    // Izin yang disetujui diwujudkan jadi baris absensi 'permit'/'sick'.
    // Tanpa ini kolom "Izin" di rekap selalu 0 dan aturan poin izin PRD tak
    // pernah berjalan — santri berizin cuma "tak punya baris".
    //
    // Best-effort: kegagalan di sini TIDAK membatalkan persetujuan yang sudah
    // tercatat. Izin tetap sah; yang tertinggal hanya baris absensinya, dan itu
    // masih bisa ditandai manual oleh guru/pamong bertugas.
    if approve {
        match repo::materialize_permit_attendance(pool, permit_id).await {
            Ok(n) if n > 0 => tracing::info!(permit_id, "izin → {n} baris absensi"),
            Ok(_) => {}
            Err(e) => tracing::warn!(permit_id, "gagal mewujudkan absensi izin: {e}"),
        }
    }
    Ok(())
}

/// Buat SATU baris izin, ditujukan ke wali kelas KBM santri.
///
/// Namanya masih "split" karena sejarahnya (migrasi 46 memecah izin per wali),
/// tapi sejak migrasi 65 tak ada lagi yang perlu dipecah: wali kelas HANYA ada
/// di kelas KBM, dan satu santri hanya punya satu kelas KBM. Berapa pun kelas
/// yang terlewat — piket, apel, Bacaan sekaligus — penyetujunya satu orang, dan
/// santri cukup mengajukan sekali.
///
/// Santri tanpa kelas KBM (baru masuk, atau hanya ikut piket) tetap dapat baris
/// izin, hanya tanpa wali — keputusannya naik ke dewan guru/admin. Izin tak
/// boleh hilang diam-diam hanya karena penempatan kelasnya belum rapi.
///
/// Return satu `PermitSplit` (tetap `Vec` supaya pemanggilnya tak berubah):
/// dipakai menyusun notifikasi yang menyebut kelas mana saja yang terlewat.
pub async fn split_permit_per_wali(
    pool: &Pool,
    student_id: i64,
    requested_by: i64,
    kind: &str,
    start_date: chrono::NaiveDate,
    end_date: Option<chrono::NaiveDate>,
    // `jam`: berlaku tiap hari dalam rentang; None = sehari penuh (migrasi 66).
    jam: Option<(chrono::NaiveTime, chrono::NaiveTime)>,
    reason: &str,
) -> Result<Vec<PermitSplit>> {
    // end_date None = izin sehari → rentang [start, start].
    let range_end = end_date.unwrap_or(start_date);
    let affected = repo::affected_classes(pool, student_id, start_date, range_end, jam).await?;

    // Saring pola recurrence-nya lebih dulu: `affected` berisi jadwal yang
    // rentang BERLAKUNYA bersinggungan dengan izin, dan itu tak sama dengan
    // benar-benar berlangsung. Jadwal Senin berlaku sepanjang semester, jadi
    // izin hari Selasa dulu ikut menyeret kelas yang hari itu tak ada kelasnya.
    //
    // Satu kelas bisa punya beberapa jadwal; cukup SATU yang jatuh di rentang.
    let mut terdampak: Vec<&repo::AffectedClass> = Vec::new();
    let mut sudah: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for c in &affected {
        let habis = c.sched_end.map_or(range_end, |e| e.min(range_end));
        let mulai = start_date.max(c.sched_start);
        if mulai > habis
            || crate::service::kelas::dates_in_range(
                &c.recurrence_type,
                c.sched_start,
                &crate::service::kelas::parse_dates(&c.custom_dates),
                mulai,
                habis,
            )
            .is_empty()
        {
            continue;
        }
        if sudah.insert(c.class_id) {
            terdampak.push(c);
        }
    }

    // Kelas acuan: kelas KBM santri — DICARI LANGSUNG, bukan dipungut dari
    // kelas yang kebetulan terlewat.
    //
    // Bedanya menentukan. Dulu barisnya `terdampak.iter().find(|c|
    // c.wali_kelas_id.is_some())`: izin yang hanya menyentuh sholat, apel, atau
    // piket — yang sejak migrasi 65 memang tak punya wali — lahir tanpa kelas
    // acuan sama sekali. Tanpa itu tak ada wali yang dituju, dan antrean pamong
    // yang mencocokkan pamong kelas acuan tak menemukan siapa pun. Izinnya ada
    // di basis data tapi tak muncul di layar mana pun.
    //
    // Sekarang acuannya selalu kelas KBM santri, jadi setelan dua-tahapnya pun
    // ikut dari sana — persis seperti kelas KBM-nya sendiri yang ditinggalkan,
    // walau yang terlewat cuma apel malam.
    let kbm = repo::kelas_kbm_santri(pool, student_id).await?;

    let permit_id = repo::insert_permit(
        pool,
        student_id,
        requested_by,
        kind,
        start_date,
        end_date,
        jam,
        reason,
        kbm.as_ref().map(|c| c.class_id),
        kbm.as_ref().and_then(|c| c.wali_kelas_id),
    )
    .await?;

    // Cakupannya SELURUH kelas terdampak (migrasi 64) — termasuk piket, apel,
    // dan Bacaan yang tak punya wali. Satu keputusan wali KBM membebaskan
    // santri dari auto-alpa di semua kelas itu; tanpa baris-baris ini, kelas
    // tanpa wali tak akan pernah tahu izinnya sudah disetujui.
    let ids: Vec<i64> = terdampak.iter().map(|c| c.class_id).collect();
    repo::insert_permit_classes(pool, permit_id, &ids).await?;

    Ok(vec![PermitSplit {
        permit_id,
        class_id: kbm.as_ref().map(|c| c.class_id),
        class_names: terdampak.iter().map(|c| c.class_name.clone()).collect(),
        wali_name: kbm.and_then(|c| c.wali_name),
    }])
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

/// Ingatkan pamong kelas lewat WhatsApp bahwa sesi KBM mulai ~1 jam lagi dan
/// guru/pamong bertugasnya belum ditunjuk.
///
/// Ditaruh di modul ini karena satu-satunya pemakai `send_wa_text` di luar
/// registrasi memang notifikasi ke petugas. Return jumlah pesan terkirim.
///
/// Sesi yang guru DAN pamongnya sudah lengkap dilewati: pengingat untuk hal
/// yang sudah dikerjakan hanya melatih orang mengabaikan pesan berikutnya.
pub async fn ingatkan_pamong_sesi(
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    pool: &Pool,
    dari_menit: i32,
    sampai_menit: i32,
) -> Result<i64> {
    let sesi = repo::sesi_perlu_pengingat(pool, dari_menit, sampai_menit).await?;
    let mut terkirim = 0i64;
    for s in sesi {
        if s.ada_guru && s.ada_pamong_sesi {
            // Sudah lengkap — tandai supaya tak diperiksa lagi tiap tick.
            let _ = repo::tandai_pengingat_terkirim(pool, s.session_id).await;
            continue;
        }
        let kurang = match (s.ada_guru, s.ada_pamong_sesi) {
            (false, false) => "guru pengajar dan pamong bertugas",
            (true, false) => "pamong bertugas",
            _ => "guru pengajar",
        };
        let msg = format!(
            "⏰ *Sesi KBM 1 jam lagi*\n{} — {}\nJam {} WIB\n\nBelum ada {}. \
             Mohon ditunjuk lewat aplikasi AFM SMART sebelum sesi dimulai, agar \
             absensinya bisa diverifikasi petugas yang tepat.",
            s.class_name, s.title, s.jam, kurang
        );
        if super::registration::send_wa_text(http, waha, &wa_phone(&s.pamong_phone), &msg)
            .await
            .is_ok()
        {
            // Ditandai HANYA setelah terkirim — kalau ditandai lebih dulu dan
            // WA-nya gagal, pamongnya tak akan pernah diingatkan.
            let _ = repo::tandai_pengingat_terkirim(pool, s.session_id).await;
            terkirim += 1;
        } else {
            tracing::warn!(
                session_id = s.session_id,
                pamong = %s.pamong_name,
                "pengingat sesi gagal terkirim — akan dicoba lagi tick berikutnya"
            );
        }
    }
    Ok(terkirim)
}

// ── Detail satu izin, untuk semua peran ──────────────────────────────────────

/// Detail izin + wewenang pemirsa. Satu jalur untuk santri, orang tua, wali
/// kelas, dan admin.
///
/// AKSES: santri pemilik, orang tua yang terhubung dengannya, wali kelas
/// tujuan, dan peran pengawas (admin/ketua/dewan guru). Selain itu ditolak —
/// isi izin menyebut alasan pribadi santri.
///
/// WEWENANG UBAH lebih sempit dari akses: hanya santri pemilik dan wali kelas,
/// dan hanya selama wali kelas belum memutuskan. Orang tua boleh MENGAJUKAN
/// (lihat `parent::submit_child_permit`) tapi tidak mengubah — begitu izin
/// berjalan, yang berwenang santri dan walinya.
pub async fn permit_detail(
    pool: &Pool,
    permit_id: i64,
    viewer_id: i64,
    viewer_role: &str,
) -> Result<crate::models::PermitDetail> {
    let Some(d) = repo::permit_detail(pool, permit_id).await? else {
        bail_user!("Pengajuan izin tidak ditemukan.");
    };

    let pengawas = crate::models::role_satisfies(viewer_role, &["admin"])
        || viewer_role == "dewan_guru";
    let pemilik = d.user_id == viewer_id;
    let wali = d.wali_kelas_id == Some(viewer_id);
    let ortu = !pemilik
        && !wali
        && !pengawas
        && repo::is_connected(pool, viewer_id, d.user_id).await.unwrap_or(false);
    if !(pemilik || wali || pengawas || ortu) {
        bail_user!("forbidden");
    }

    let (status_label, status_kind) =
        crate::models::permit_stage(&d.pamong_status, &d.guru_status, d.require_pamong);
    let diputus = d.guru_status != "pending";
    let can_edit = (pemilik || wali) && !diputus;
    let lock_reason = if diputus {
        // Alasannya konkret: absensinya sudah terlanjur diwujudkan, jadi
        // mengubah tanggal setelah itu meninggalkan baris izin di sesi yang
        // tak lagi tercakup.
        "Sudah diputuskan wali kelas — isinya terkunci.".to_string()
    } else if !(pemilik || wali) {
        "Hanya santri yang bersangkutan dan wali kelasnya yang boleh mengubah.".to_string()
    } else {
        String::new()
    };

    let dampak = repo::dampak_izin(pool, &[permit_id]).await.unwrap_or_default();
    let (sesi_terlewat, total_sesi) = dampak.get(&permit_id).cloned().unwrap_or_default();

    let oleh_ortu = d.requested_by != d.user_id && d.requester_role == "parent";
    let diajukan_oleh = if d.requested_by == d.user_id {
        "Diajukan santri sendiri".to_string()
    } else if oleh_ortu {
        format!("Diajukan orang tua — {}", d.requester_name)
    } else {
        format!("Diajukan oleh {}", d.requester_name)
    };

    Ok(crate::models::PermitDetail {
        id: d.id,
        student_name: d.student_name,
        kind_label: crate::models::permit_kind_label(&d.kind).into(),
        kind: d.kind,
        reason: d.reason,
        range_label: fmt_range(d.start_date, d.end_date),
        jam_label: match (d.start_time, d.end_time) {
            (Some(a), Some(b)) => format!("{} – {} WIB", a.format("%H:%M"), b.format("%H:%M")),
            _ => String::new(),
        },
        start_date: d.start_date.to_string(),
        end_date: d.end_date.map(|x| x.to_string()).unwrap_or_default(),
        jam_mulai: d.start_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_default(),
        jam_selesai: d.end_time.map(|t| t.format("%H:%M").to_string()).unwrap_or_default(),
        status_label: status_label.into(),
        status_kind: status_kind.into(),
        class_label: d.class_name.unwrap_or_default(),
        wali_name: d.wali_name.unwrap_or_default(),
        sesi_terlewat,
        total_sesi,
        diajukan_oleh,
        oleh_ortu,
        when_label: fmt_when(d.created_at),
        can_edit,
        lock_reason,
    })
}

/// Ubah pengajuan izin yang masih menunggu keputusan.
///
/// Cakupan kelasnya DIHITUNG ULANG: mengubah tanggal/jam mengubah kelas mana
/// yang terlewat, dan baris cakupan lama yang tertinggal akan terus
/// membebaskan santri dari auto-alpa di kelas yang sudah di luar izinnya.
#[allow(clippy::too_many_arguments)]
pub async fn update_permit(
    pool: &Pool,
    permit_id: i64,
    actor_id: i64,
    kind: &str,
    start: &str,
    end: &str,
    jam_mulai: &str,
    jam_selesai: &str,
    reason: &str,
) -> Result<()> {
    let (kind, start_date, end_date, jam, reason) =
        super::santri::validasi_izin(kind, start, end, jam_mulai, jam_selesai, reason)?;

    if !repo::update_permit(pool, permit_id, actor_id, kind, start_date, end_date, jam, &reason)
        .await?
    {
        bail_user!(
            "Izin ini tak bisa diubah — sudah diputuskan wali kelas, atau bukan izin Anda."
        );
    }

    // Cakupan dihitung ulang dengan aturan yang sama persis dengan pengajuan
    // baru (saringan recurrence + jam), lalu ditulis ganti-seluruhnya.
    let Some(d) = repo::permit_detail(pool, permit_id).await? else {
        return Ok(());
    };
    let range_end = end_date.unwrap_or(start_date);
    let affected = repo::affected_classes(pool, d.user_id, start_date, range_end, jam).await?;
    let mut sudah: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut ids: Vec<i64> = Vec::new();
    for c in &affected {
        let habis = c.sched_end.map_or(range_end, |e| e.min(range_end));
        let mulai = start_date.max(c.sched_start);
        if mulai > habis
            || crate::service::kelas::dates_in_range(
                &c.recurrence_type,
                c.sched_start,
                &crate::service::kelas::parse_dates(&c.custom_dates),
                mulai,
                habis,
            )
            .is_empty()
        {
            continue;
        }
        if sudah.insert(c.class_id) {
            ids.push(c.class_id);
        }
    }
    repo::ganti_cakupan_izin(pool, permit_id, &ids).await?;
    Ok(())
}
