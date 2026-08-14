//! service/permits.rs — Pengajuan & antrean izin.
//!
//! Migrasi 46 — izin PER-KELAS. Satu pengajuan dipecah jadi beberapa baris
//! `permit_requests`, satu untuk tiap WALI KELAS yang kelasnya dilewati selama
//! rentang izin (lihat `split_permit_per_wali`). Tiap baris diputus WALI KELAS
//! kelas itu — satu tahap.
//!
//! Tahap PAMONG (dulu tahap-1) sudah tidak ada: perannya dihapus dan semua
//! kelas dipindah ke `verify_mode='guru'` (migrasi 84), jadi `require_pamong`
//! bernilai FALSE di mana-mana. Cabang pamong di query & fungsi di bawah
//! sengaja dibiarkan — dengan datanya kosong, cabang itu tak pernah terpilih,
//! dan izin lama yang terlanjur menggantung di tahap-1 tetap bisa difinalkan
//! wali kelas.
//!
//! Orang tua BUKAN penyetuju lagi — mereka hanya dinotifikasi & bisa melihat.


use anyhow::Result;
use deadpool_postgres::Pool;

use super::fmt::{fmt_rentang, fmt_when};
use crate::models::{permit_kind_label, PermitQueueData, PermitReviewItem, SedangIzinItem};
use crate::repository as repo;

/// Normalisasi HP untuk chat-ID WAHA — satu aturan bersama
/// ([`crate::models::normalisasi_hp`]).
fn wa_phone(p: &str) -> String {
    crate::models::normalisasi_hp(p).unwrap_or_default()
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
        if let Some(phone) = t.wali_phone.as_deref().filter(|p| !p.is_empty()) {
            let _ = super::registration::send_wa_text(http, waha, &wa_phone(phone), &msg).await;
        }
    }
}

/// Tanggal-tanggal saat sebuah JADWAL benar-benar terlewat oleh sebuah izin.
///
/// Inilah satu-satunya tempat aturan "izin itu rentang waktu, bukan jam yang
/// berulang" ditulis — dipakai pratinjau, pengajuan, dan penyuntingan, supaya
/// ketiganya mustahil menjawab berbeda.
///
/// Tanpa jam (izin sehari penuh): semua tanggal jadwal di dalam rentang.
/// Dengan jam: izin membentang dari (tanggal mulai + jam keluar) sampai
/// (tanggal selesai + jam pulang), jadi
///   • hari PERTAMA  → hanya kelas yang BERAKHIR setelah jam keluar;
///   • hari TERAKHIR → hanya kelas yang MULAI sebelum jam pulang;
///   • hari di ANTARANYA → semuanya (santrinya memang tak ada di pondok);
///   • izin SEHARI (mulai = selesai) → kelas yang jamnya bersinggungan saja.
pub(crate) fn tanggal_izin(
    c: &repo::AffectedClass,
    mulai: chrono::NaiveDateTime,
    selesai: chrono::NaiveDateTime,
) -> Vec<chrono::NaiveDate> {
    let habis = c.sched_end.map_or(selesai.date(), |e| e.min(selesai.date()));
    let awal = mulai.date().max(c.sched_start);
    if awal > habis {
        return Vec::new();
    }
    crate::service::kelas::dates_in_range(
        &c.recurrence_type,
        c.sched_start,
        &crate::service::kelas::parse_dates(&c.custom_dates),
        awal,
        habis,
    )
    .into_iter()
    // SATU aturan untuk semua hari: kelas terlewat bila jamnya beririsan
    // dengan rentang izin. Tak ada lagi perlakuan khusus hari pertama/terakhir
    // — itu hanya perlu ketika izin masih berupa "jam yang berulang tiap hari".
    .filter(|d| d.and_time(c.jam_mulai) < selesai && d.and_time(c.jam_selesai) > mulai)
    .collect()
}

/// Baris antrean + DAMPAKNYA: kelas apa saja yang terlewat bila disetujui.
///
/// Wali kelas memutuskan sambil melihat akibatnya. Rentang tanggal saja tak
/// menjawab "berapa kelas yang ia tinggalkan" — dan izin dua hari bisa berarti
/// dua sesi atau sepuluh, tergantung jadwalnya.
///
/// Dampaknya DIHITUNG ULANG dari irisan waktu (migrasi 86), bukan dibaca dari
/// daftar kelas tersimpan. Tabel cakupan itu dibuang karena sejak izin berupa
/// satu rentang waktu, jawabannya sama setiap kali dihitung — sementara tabel
/// perantara justru bisa basi ketika jadwal kelasnya berubah.
async fn to_review_items(pool: &Pool, rows: Vec<repo::PermitQueueRow>) -> Vec<PermitReviewItem> {
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
                // Jamnya sudah termuat di `range_label` (lihat `fmt_rentang`),
                // jadi tak ada lagi yang perlu disebut terpisah.
                range_label: fmt_rentang(p.mulai, p.selesai),
                jam_label: String::new(),
                sesi_terlewat: d.map(|v| v.0.clone()).unwrap_or_default(),
                total_sesi: d.map(|v| v.1).unwrap_or(0),
                reason: p.reason,
                when_label: fmt_when(p.created_at),
            }
        })
        .collect()
}

/// Payload /izin-staf: izin santri di kelas yang diampu peninjau.
///
/// Parameter `role` DIBUANG (Ags 2026): sejak cabang pamong hilang, antreannya
/// sama untuk setiap peran yang boleh membuka layar ini — yang membedakan
/// hanyalah `user_id`, karena wali kelas hanya melihat izin kelasnya sendiri.
/// Membiarkan parameter yang tak lagi menentukan apa pun mengundang pemanggil
/// berikutnya mengira ada percabangan yang sebenarnya tidak ada.
pub async fn permit_queue(pool: &Pool, user_id: i64) -> Result<PermitQueueData> {
    // Guru hanya melihat izin santri di KELAS YANG IA AMPU — bukan semua.
    //
    // Dulu `wali_id` hanya diisi untuk role "teacher"; padahal peran itu sudah
    // digabung ke "dewan_guru" (migrasi 36), jadi praktis SELALU None dan
    // setiap guru melihat antrean seluruh pesantren. Admin pun sengaja tak
    // sampai ke sini lagi (gate di api.rs).
    let wali_id = Some(user_id);
    let (pending, decided_today) = tokio::join!(
        repo::pending_guru_permits(pool, wali_id, 50),
        repo::guru_permits_decided_today(pool, wali_id),
    );
    let items = to_review_items(pool, pending?).await;
    Ok(PermitQueueData {
        pending_count: items.len() as i64,
        approved_today: decided_today?,
        items,
        stage_label: "Persetujuan Wali Kelas".into(),
    })
}

/// Izin ini tersimpan sebagai sehari penuh (00:00 → 23:59:59)?
pub(crate) fn sehari_penuh(
    mulai: chrono::NaiveDateTime,
    selesai: chrono::NaiveDateTime,
) -> bool {
    mulai.time() == chrono::NaiveTime::MIN
        && selesai.time() >= chrono::NaiveTime::from_hms_opt(23, 59, 0).expect("23:59 valid")
}

/// Satu baris izin aktif → payload layar. Dipakai daftar staf DAN spanduk
/// santri, supaya keduanya mustahil berbeda bentuk.
pub fn baris_sedang_izin(r: repo::IzinAktifRow, hari: chrono::NaiveDate) -> SedangIzinItem {
    let habis = r.selesai.date();
    // TERMASUK hari ini: izin yang berakhir hari ini masih berlaku hari ini,
    // dan menuliskan "0 hari lagi" pada izin yang sedang berjalan hanya
    // membingungkan yang membacanya.
    let sisa_hari = (habis - hari).num_days().max(0) + 1;
    let nis = r.nis.filter(|s| !s.is_empty()).unwrap_or_else(|| "-".into());
    SedangIzinItem {
        user_id: r.user_id,
        name: r.student_name,
        nis,
        class_name: r.class_name.filter(|s| !s.is_empty()).unwrap_or_else(|| "-".into()),
        kind_label: permit_kind_label(&r.kind).into(),
        kind: r.kind,
        range_label: fmt_rentang(r.mulai, r.selesai),
        // Jamnya sudah termuat di `range_label`; medan ini sisa bentuk lama.
        jam_label: String::new(),
        sampai_label: if habis == hari {
            "sampai hari ini".into()
        } else {
            format!("sampai {}", fmt_rentang(r.selesai, r.selesai))
        },
        sisa_hari,
        reason: r.reason,
    }
}

/// Semua santri yang sedang izin/sakit HARI INI — pantauan ketua/admin/guru.
pub async fn sedang_izin(pool: &Pool) -> Result<Vec<SedangIzinItem>> {
    let hari = super::fmt::today_wib();
    let rows = repo::izin_aktif(pool, hari, None, 300).await?;
    Ok(rows.into_iter().map(|r| baris_sedang_izin(r, hari)).collect())
}

/// Izin yang sedang berlaku untuk SATU santri (spanduk di layarnya sendiri).
pub async fn izin_aktif_saya(pool: &Pool, user_id: i64) -> Result<Option<SedangIzinItem>> {
    let hari = super::fmt::today_wib();
    // Bila kebetulan ada lebih dari satu (mis. izin pulang yang beririsan
    // dengan izin sakit), yang ditampilkan yang BERAKHIR PALING AKHIR — itu
    // yang menentukan sampai kapan ia sebenarnya tak masuk. Urutan query sudah
    // menaik menurut tanggal berakhir, jadi cukup ambil yang terakhir.
    let rows = repo::izin_aktif(pool, hari, Some(user_id), 10).await?;
    Ok(rows.into_iter().next_back().map(|r| baris_sedang_izin(r, hari)))
}

/// Setujui/tolak izin — SATU tahap: wali kelas KBM santri.
///
/// Percabangan "coba tahap pamong dulu" DIBUANG (Ags 2026). Ia lahir untuk
/// menutup ketimpangan lama (antrean pamong ada, tombolnya tak berfungsi), dan
/// sejak perannya dihapus (migrasi 84) percabangan itu justru berbahaya: pada
/// data lama yang belum tersentuh migrasi, seorang wali yang KEBETULAN juga
/// tercatat pamong kelas itu akan masuk ke cabang pamong — izinnya cuma maju
/// setengah tahap lalu tetap menggantung di antrean, dan layarnya melaporkan
/// "berhasil".
///
/// Sekarang: satu tombol, satu jalur, syarat yang SAMA PERSIS dengan penyaring
/// antreannya (lihat `repo::decide_guru_permit`). Apa pun yang tampil di
/// /izin-staf pasti bisa diputuskan orang yang melihatnya.
pub async fn decide_permit(pool: &Pool, permit_id: i64, approve: bool, staff_id: i64) -> Result<()> {
    // Wali kelas KBM santri. Bukan dewan guru, bukan admin — izin
    // adalah urusan antara santri dan walinya, dan orang yang tak mengenal
    // santrinya tak punya dasar untuk menimbang. `wali_id = staf ini`, jadi
    // izin milik wali LAIN tak akan tersentuh.
    let ok = repo::decide_guru_permit(pool, permit_id, approve, Some(staff_id), staff_id).await?;
    if !ok {
        // Pesan lama menyebut pamong & tahap dua — dua hal yang sudah tak ada
        // (migrasi 84), dan justru itulah yang membuat galatnya membingungkan:
        // ia menerangkan sebab yang bukan sebabnya.
        bail_user!(
            "Izin ini sudah diputuskan orang lain, atau bukan milik kelas yang Anda \
             ampu. Yang boleh memutuskan hanya WALI KELAS KBM santri tersebut."
        );
    }

    // Izin yang disetujui diwujudkan jadi baris absensi 'permit'/'sick'.
    // Tanpa ini kolom "Izin" di rekap selalu 0 dan aturan poin izin PRD tak
    // pernah berjalan — santri berizin cuma "tak punya baris".
    //
    // Best-effort: kegagalan di sini TIDAK membatalkan persetujuan yang sudah
    // tercatat. Izin tetap sah; yang tertinggal hanya baris absensinya, dan itu
    // masih bisa ditandai manual oleh guru bertugas di sesinya.
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
    // SATU rentang: keluar → kembali (migrasi 86).
    mulai: chrono::NaiveDateTime,
    selesai: chrono::NaiveDateTime,
    reason: &str,
) -> Result<Vec<PermitSplit>> {
    let affected =
        repo::affected_classes(pool, student_id, mulai.date(), selesai.date()).await?;

    // Saring pola recurrence-nya lebih dulu: `affected` berisi jadwal yang
    // rentang BERLAKUNYA bersinggungan dengan izin, dan itu tak sama dengan
    // benar-benar berlangsung. Jadwal Senin berlaku sepanjang semester, jadi
    // izin hari Selasa dulu ikut menyeret kelas yang hari itu tak ada kelasnya.
    //
    // Satu kelas bisa punya beberapa jadwal; cukup SATU yang jatuh di rentang.
    let mut terdampak: Vec<&repo::AffectedClass> = Vec::new();
    let mut sudah: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for c in &affected {
        if tanggal_izin(c, mulai, selesai).is_empty() {
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
    //
    // DITOLAK di depan bila santrinya belum punya kelas KBM. Satu santri wajib
    // punya tepat satu kelas KBM (trigger `trg_satu_kelas_kbm`, migrasi 65) dan
    // wali kelas itulah satu-satunya penyetuju izinnya. Menerima pengajuan tanpa
    // kelas acuan berarti melahirkan kembali bug yang dijelaskan di atas: izin
    // tersimpan rapi di basis data, tak muncul di antrean siapa pun, dan santri
    // mengira dirinya sudah berizin sampai tercatat ALFA.
    //
    // Lebih baik gagal sekarang dengan sebab yang jelas daripada berhasil
    // menyimpan sesuatu yang tak akan pernah diputus.
    let Some(kbm) = repo::kelas_kbm_santri(pool, student_id).await? else {
        bail_user!(
            "Kamu belum terdaftar di kelas KBM mana pun, jadi belum ada wali kelas \
             yang bisa menyetujui izin ini. Hubungi pengurus untuk didaftarkan dulu."
        );
    };
    let Some(wali_id) = kbm.wali_kelas_id else {
        bail_user!(
            "Kelas {} belum punya wali kelas, jadi izin ini belum ada yang bisa \
             menyetujui. Hubungi pengurus untuk menetapkan wali kelasnya dulu.",
            kbm.class_name
        );
    };

    let permit_id = repo::insert_permit(
        pool,
        student_id,
        requested_by,
        kind,
        mulai,
        selesai,
        reason,
        Some(kbm.class_id),
        Some(wali_id),
    )
    .await?;

    // Cakupan kelas TIDAK lagi disimpan (`permit_request_classes` dihapus,
    // migrasi 86): sejak izin berupa rentang waktu, kelas yang terlewat bisa
    // dihitung kapan saja dari irisan jadwal × rentang — jawabannya sama setiap
    // kali, tanpa tabel perantara yang bisa basi saat jadwal berubah.

    Ok(vec![PermitSplit {
        permit_id,
        class_id: Some(kbm.class_id),
        class_names: terdampak.iter().map(|c| c.class_name.clone()).collect(),
        wali_name: kbm.wali_name,
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
pub async fn ingatkan_wali_sesi(
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    pool: &Pool,
    dari_menit: i32,
    sampai_menit: i32,
) -> Result<i64> {
    let sesi = repo::sesi_perlu_pengingat(pool, dari_menit, sampai_menit).await?;
    let mut terkirim = 0i64;
    for s in sesi {
        if s.ada_guru {
            // Sudah lengkap — tandai supaya tak diperiksa lagi tiap tick.
            let _ = repo::tandai_pengingat_terkirim(pool, s.session_id).await;
            continue;
        }
        let msg = format!(
            "⏰ *Sesi KBM 1 jam lagi*\n{} — {}\nJam {} WIB\n\nBelum ada guru pengajar. \
             Mohon ditunjuk lewat aplikasi AFM SMART sebelum sesi dimulai, agar \
             absensinya bisa diverifikasi petugas yang tepat.",
            s.class_name, s.title, s.jam
        );
        if super::registration::send_wa_text(http, waha, &wa_phone(&s.wali_phone), &msg)
            .await
            .is_ok()
        {
            // Ditandai HANYA setelah terkirim — kalau ditandai lebih dulu dan
            // WA-nya gagal, walinya tak akan pernah diingatkan.
            let _ = repo::tandai_pengingat_terkirim(pool, s.session_id).await;
            terkirim += 1;
        } else {
            tracing::warn!(
                session_id = s.session_id,
                wali = %s.wali_name,
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
        crate::models::permit_stage(&d.guru_status);
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
        range_label: fmt_rentang(d.mulai, d.selesai),
        jam_label: String::new(),
        // Pra-isi form sunting: tanggal & jam dipisah lagi di sini karena
        // <input type="date"> dan <input type="time"> memang dua kotak.
        start_date: d.mulai.date().to_string(),
        end_date: d.selesai.date().to_string(),
        // Sehari penuh (00:00 → 23:59:59) dikembalikan sebagai jam KOSONG:
        // form-nya memang mengartikan kosong sebagai "sehari penuh", dan
        // memajang 00:00/23:59 di sana membuat orang mengira ada jam yang
        // sengaja dipilih.
        jam_mulai: if sehari_penuh(d.mulai, d.selesai) {
            String::new()
        } else {
            d.mulai.format("%H:%M").to_string()
        },
        jam_selesai: if sehari_penuh(d.mulai, d.selesai) {
            String::new()
        } else {
            d.selesai.format("%H:%M").to_string()
        },
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
    let (kind, mulai, selesai, reason) =
        super::santri::validasi_izin(kind, start, end, jam_mulai, jam_selesai, reason)?;

    if !repo::update_permit(pool, permit_id, actor_id, kind, mulai, selesai, &reason).await? {
        bail_user!(
            "Izin ini tak bisa diubah — sudah diputuskan wali kelas, atau bukan izin Anda."
        );
    }
    // Cakupan kelas tak perlu ditulis ulang: sejak izin berupa rentang waktu,
    // ia dihitung dari rentangnya sendiri setiap kali dibutuhkan.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};

    fn tgl(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }
    fn jam(s: &str) -> NaiveTime {
        NaiveTime::parse_from_str(s, "%H:%M").unwrap()
    }
    fn dt(t: &str, j: &str) -> chrono::NaiveDateTime {
        tgl(t).and_time(jam(j))
    }

    /// Kelas harian 08:00–10:00, berlaku sepanjang Agustus 2026.
    fn kelas_harian() -> repo::AffectedClass {
        repo::AffectedClass {
            class_id: 1,
            class_name: "Nahwu".into(),
            wali_kelas_id: Some(9),
            wali_name: Some("Ust. A".into()),
            recurrence_type: "daily".into(),
            sched_start: tgl("2026-08-01"),
            sched_end: Some(tgl("2026-08-31")),
            custom_dates: Vec::new(),
            jam_mulai: jam("08:00"),
            jam_selesai: jam("10:00"),
        }
    }

    /// REGRESI TERPENTING (migrasi 86): izin multi-hari menutup hari-hari di
    /// TENGAHNYA secara penuh.
    ///
    /// Model lama memperlakukan jam izin sebagai "jam berlaku pada setiap
    /// hari", sehingga santri yang pulang Jumat 14:00 dan kembali Minggu 08:00
    /// tetap tercatat ALFA sepanjang Sabtu — kelas Sabtu 08:00 dianggap di luar
    /// jendela 14:00–08:00. Padahal ia jelas tak ada di pondok.
    #[test]
    fn hari_tengah_izin_tertutup_penuh() {
        let k = kelas_harian();
        let hari = tanggal_izin(&k, dt("2026-08-07", "14:00"), dt("2026-08-09", "08:30"));
        assert!(
            hari.contains(&tgl("2026-08-08")),
            "hari tengah wajib ikut terlewat, dapat {hari:?}"
        );
    }

    /// Ujung rentang dipotong menurut JAM, bukan menurut tanggal.
    #[test]
    fn ujung_rentang_dipotong_menurut_jam() {
        let k = kelas_harian();
        // Keluar 14:00 → kelas 08:00–10:00 hari itu SUDAH lewat, tak terlewat.
        // Kembali 08:30 → kelas hari itu sudah berjalan saat ia tiba, terlewat.
        let hari = tanggal_izin(&k, dt("2026-08-07", "14:00"), dt("2026-08-09", "08:30"));
        assert!(!hari.contains(&tgl("2026-08-07")), "hari keluar: kelas sudah selesai");
        assert!(hari.contains(&tgl("2026-08-09")), "hari kembali: kelas sedang berjalan");
    }

    /// Bersentuhan di ujung bukan beririsan: kembali TEPAT saat kelas mulai
    /// berarti ia sempat mengikutinya.
    #[test]
    fn sentuhan_ujung_bukan_irisan() {
        let k = kelas_harian();
        // Kembali tepat 08:00 = tepat saat kelas dimulai → masih terkejar.
        let hari = tanggal_izin(&k, dt("2026-08-07", "14:00"), dt("2026-08-09", "08:00"));
        assert!(!hari.contains(&tgl("2026-08-09")));
        // Keluar tepat 10:00 = tepat saat kelas bubar → sudah diikuti.
        let hari = tanggal_izin(&k, dt("2026-08-07", "10:00"), dt("2026-08-07", "16:00"));
        assert!(hari.is_empty(), "dapat {hari:?}");
    }

    /// Izin sehari penuh (00:00 → 23:59:59) menutup seluruh kelas hari itu.
    #[test]
    fn sehari_penuh_menutup_semua_kelas_hari_itu() {
        let k = kelas_harian();
        let akhir = NaiveTime::from_hms_opt(23, 59, 59).unwrap();
        let hari = tanggal_izin(
            &k,
            tgl("2026-08-10").and_time(NaiveTime::MIN),
            tgl("2026-08-10").and_time(akhir),
        );
        assert_eq!(hari, vec![tgl("2026-08-10")]);
    }

    /// Rentang izin dipotong masa berlaku jadwal — kelas yang belum/sudah tak
    /// berjalan tak boleh dihitung terlewat.
    #[test]
    fn di_luar_masa_berlaku_jadwal_tak_dihitung() {
        let mut k = kelas_harian();
        k.sched_end = Some(tgl("2026-08-05"));
        let hari = tanggal_izin(&k, dt("2026-08-07", "00:00"), dt("2026-08-09", "23:59"));
        assert!(hari.is_empty(), "jadwal sudah berakhir, dapat {hari:?}");

        k.sched_start = tgl("2026-08-20");
        k.sched_end = None;
        let hari = tanggal_izin(&k, dt("2026-08-07", "00:00"), dt("2026-08-09", "23:59"));
        assert!(hari.is_empty(), "jadwal belum mulai, dapat {hari:?}");
    }

    /// Jadwal 'custom' (dipakai SELURUH KBM di produksi) ikut terhitung.
    #[test]
    fn jadwal_custom_ikut_terhitung() {
        let mut k = kelas_harian();
        k.recurrence_type = "custom".into();
        k.custom_dates = vec!["2026-08-08".into(), "2026-08-15".into()];
        let hari = tanggal_izin(&k, dt("2026-08-07", "14:00"), dt("2026-08-09", "08:30"));
        assert_eq!(hari, vec![tgl("2026-08-08")]);
    }
}
