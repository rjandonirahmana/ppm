//! service/attendance.rs — Alur absensi: scan RFID + verifikasi pamong.

use anyhow::Result;
use chrono::Utc;
use deadpool_postgres::Pool;

use super::fmt::{fmt_when, wib};
use crate::models::{PamongData, PendingAtt, RfidScanRequest, RfidScanResponse};
use crate::repository as repo;

/// Error scan dibedakan agar handler bisa memberi kode HTTP yang tepat.
pub enum ScanError {
    BadApiKey,
    UnknownCard,
    Db(anyhow::Error),
}

impl From<anyhow::Error> for ScanError {
    fn from(e: anyhow::Error) -> Self {
        ScanError::Db(e)
    }
}

/// Proses satu tap kartu. PERANGKAT yang menentukan perilaku (migrasi 49):
///   • kategori `gate_utama` → toggle KELUAR/MASUK area pondok. Bukan absensi.
///   • kategori lain → absensi kelas: cocokkan jadwal aktif santri →
///     present/late → simpan (dedup per hari). Tap di luar jadwal tetap
///     tercatat sbg log gerbang (schedule NULL).
///
/// Pencocokan jadwal TIDAK terikat perangkat — satu jadwal bisa di-tap di
/// perangkat mana pun (selain gate_utama). Kolom `class_schedules.room_id`
/// hanya keterangan ruang, tak dipakai saat scan.
pub async fn record_scan(
    pool: &Pool,
    redis: &mut redis::aio::ConnectionManager,
    req: &RfidScanRequest,
) -> Result<RfidScanResponse, ScanError> {
    let Some(device) = repo::find_device_by_key(pool, &req.api_key).await? else {
        return Err(ScanError::BadApiKey);
    };

    let Some((user_id, name)) = repo::find_user_by_card(pool, req.card).await? else {
        // Kartu asing tetap DITOLAK, tapi nomornya dititipkan supaya admin bisa
        // memasangkannya ke pengguna tanpa mengetik 10 digit (lihat
        // service::enrollment). Inilah satu-satunya jalur pendaftaran kartu.
        super::enrollment::remember_unknown_card(redis, req.card, &device.device_name).await;
        return Err(ScanError::UnknownCard);
    };

    // GERBANG UTAMA: penanda orangnya keluar/masuk area pondok, BUKAN kehadiran
    // kelas. Firmware tak perlu tahu bedanya — satu build yang selalu POST ke
    // /api/rfid/scan sudah cukup; kategori perangkat yang memutuskan. Ganti
    // peran perangkat = ubah kategori di admin, tanpa flash ulang.
    if crate::models::is_main_gate(&device.category) {
        let direction = repo::toggle_gate(pool, user_id, Some(device.id)).await?;
        let message = if direction == "out" {
            "keluar area pondok"
        } else {
            "masuk area pondok"
        };
        tracing::info!(user_id, card = req.card, gate = %device.device_name, direction,
            "gerbang utama: keluar/masuk area");
        return Ok(RfidScanResponse {
            ok: true,
            message: message.into(),
            student: Some(name),
            // Diawali "gate_" supaya jelas BUKAN status absensi kelas
            // (present/late/…) — firmware & log tak salah tafsir.
            status: Some(format!("gate_{direction}")),
        });
    }

    // Salin nama sebelum `device` dipecah — masih dipakai untuk log penolakan.
    let device_name = device.device_name.clone();
    let gate = device.location.unwrap_or(device.device_name);

    // Jadwal pesantren dicatat dalam waktu lokal (WIB).
    let now = Utc::now().with_timezone(&wib());
    let today = now.date_naive();
    let now_time = now.time();

    // Jadwal dicocokkan JUGA dengan perangkatnya: jadwal yang ruangnya diisi
    // (class_schedules.room_id) hanya sah di-tap di perangkat itu. Tanpa ini,
    // santri yang mestinya di masjid bisa menempel kartu di gedung putra dan
    // tetap terhitung hadir. Jadwal tanpa ruang tetap bebas di-tap di mana pun.
    let schedule = repo::active_schedule_now(pool, user_id, today, now_time, device.id).await?;
    let (schedule_id, session_id, status, note) = match &schedule {
        Some(s) => {
            let st = if now_time <= s.limit_entry {
                "present"
            } else {
                "late"
            };
            (Some(s.id), Some(s.session_id), st, None)
        }
        None => {
            // SALAH RUANG → TOLAK, jangan catat apa pun. Santri yang jadwalnya
            // di masjid lalu menempel kartu di gedung putra tidak boleh
            // meninggalkan jejak absensi apa pun — termasuk baris
            // `outside_schedule`, yang di rekap mingguan ikut terhitung sebagai
            // "telat" dan akan mengaburkan data.
            //
            // Ini HANYA berlaku bila jadwalnya memang terikat ruang. Jadwal
            // tanpa ruang (room_id NULL = "bebas/ALL") sudah lolos di query di
            // atas, jadi tak pernah sampai sini.
            if let Some(room) =
                repo::active_schedule_room_elsewhere(pool, user_id, today, now_time, device.id)
                    .await
                    .unwrap_or(None)
            {
                tracing::info!(
                    user_id, card = req.card, device = %device_name, %room,
                    "tap DITOLAK: bukan perangkat kelas yang bersangkutan"
                );
                return Ok(RfidScanResponse {
                    ok: false,
                    message: format!("Salah tempat — kelasmu di {room}. Absen tidak dicatat."),
                    student: Some(name),
                    status: Some("wrong_room".into()),
                });
            }
            // Tak ada jadwal aktif sama sekali → tetap dicatat sbg log gerbang
            // (perilaku lama, disengaja: jejak lalu-lalang tetap berguna).
            (None, None, "outside_schedule", Some("scan di luar jadwal".to_string()))
        }
    };

    // Dedup: satu catatan per jadwal (atau per hari untuk scan bebas).
    if repo::attendance_exists_today(pool, user_id, schedule_id, today).await? {
        return Ok(RfidScanResponse {
            ok: true,
            message: "sudah tercatat sebelumnya".into(),
            student: Some(name),
            status: Some(status.into()),
        });
    }

    // Sesi tak perlu dicari lagi: `active_schedule_now` hanya cocok bila sesi
    // hari ini memang ada, jadi id-nya sudah terbawa dari sana. Dulu ini query
    // kedua yang bisa mengembalikan None — absensi lalu tercatat tanpa tertaut
    // sesi mana pun, dan tak pernah muncul di panel verifikasi guru.

    // None = kalah balapan dgn tap kembar (ON CONFLICT). Bukan galat: mesin
    // cukup diberi tahu absennya sudah ada.
    if repo::insert_attendance(
        pool,
        user_id,
        session_id,
        schedule_id,
        device.id,
        &gate,
        status,
        note.as_deref(),
    )
    .await?
    .is_none()
    {
        return Ok(RfidScanResponse {
            ok: true,
            message: "sudah tercatat sebelumnya".into(),
            student: Some(name),
            status: Some(status.into()),
        });
    }

    tracing::info!(user_id, card = req.card, gate = %gate, status, "RFID scan tercatat");
    Ok(RfidScanResponse {
        ok: true,
        message: "tercatat".into(),
        student: Some(name),
        status: Some(status.into()),
    })
}

/// Data halaman verifikasi pamong: antrean + statistik hari ini + sesi hari ini
/// + kehadiran terbaru (dashboard pamong ala mockup).
pub async fn pamong_data(pool: &Pool, pamong_id: Option<i64>) -> Result<PamongData> {
    let (pending, approved_today, stats, today, latest) = tokio::join!(
        repo::pending_pamong(pool, pamong_id, 50),
        repo::approved_today(pool),
        repo::staf_stats(pool),
        repo::today_sessions(pool, 5),
        repo::latest_attendance(pool, 6),
    );

    let pending = pending?
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

    let (total_santri, _growth, hadir_today, _izin) = stats?;
    let pct = if total_santri > 0 {
        ((hadir_today * 100) / total_santri) as i32
    } else {
        0
    };

    Ok(PamongData {
        pending,
        approved_today: approved_today?,
        total_santri,
        hadir_today,
        pct,
        today: super::dashboard::map_live(today?),
        latest: super::dashboard::map_latest(latest?),
    })
}

/// Setujui/tolak satu absensi (tahap pamong). `pamong_id` Some = guard hanya
/// kelas yang diampu guru ini (migrasi 30).
///
/// MENOLAK di sini juga MENARIK poin absensi itu bila tahap final sudah
/// terlanjur memberikannya — tahap final memang boleh mendahului pamong (lihat
/// `repo::decide_pamong`). Jadi ini bukan sekadar "ubah status".
pub async fn decide_pamong(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    pamong_id: Option<i64>,
) -> Result<bool> {
    repo::decide_pamong(pool, att_id, approver, approve, pamong_id).await
}

// ── Verifikasi TAHAP 2 (dewan guru) ──────────────────────────────────────────────

/// Antrean verifikasi final + jumlah terverifikasi hari ini. Reuse PamongData
/// (pending + count) — `approved_today` di sini bermakna "terverifikasi hari ini".
pub async fn verify_data(pool: &Pool, teacher_id: Option<i64>) -> Result<PamongData> {
    let (pending, verified_today, stats) = tokio::join!(
        repo::pending_verify(pool, teacher_id, 50),
        repo::verified_today(pool),
        repo::staf_stats(pool),
    );
    let pending = pending?
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
    let (total_santri, _g, hadir_today, _i) = stats?;
    let pct = if total_santri > 0 {
        ((hadir_today * 100) / total_santri) as i32
    } else {
        0
    };
    Ok(PamongData {
        pending,
        approved_today: verified_today?,
        total_santri,
        hadir_today,
        pct,
        today: vec![],
        latest: vec![],
    })
}

/// Verifikasi final satu absensi (tahap final, oleh ustad bertugas). `teacher_id`
/// Some = guard hanya sesi yang ustadnya guru ini (migrasi 33).
///
/// Jalur SATUAN ini hanya dipakai peran guru/dewan_guru/admin (lihat server fn
/// `decide_verify`), jadi pembandingnya selalu kolom GURU — pamong memutuskan
/// lewat `decide_session` yang membawa penandanya sendiri.
pub async fn decide_verify(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    teacher_id: Option<i64>,
) -> Result<bool> {
    repo::decide_verify(pool, att_id, approver, approve, teacher_id, false).await
}

// ── Verifikasi kehadiran PER-SESI (batch) ─────────────────────────────────────
// Tahap ditentukan dari PERAN: supervisor → pamong (hanya sesi yg ia pamong);
// dewan_guru/admin → final (semua sesi). Klien kirim SATU request per sesi;
// server melakukan approve semua yang pending KECUALI `reject_ids`.

use crate::models::{SessionVerifyData, SessionVerifyItem};

/// (tahap, label, id petugas yang membatasi daftar, petugas itu PAMONG?).
///
/// Elemen keempat menentukan kolom pembanding `actor`: pamong sesi atau guru
/// sesi. Tanpa itu, id pamong pernah diadu dengan kolom guru — cocok tak
/// pernah, atau lebih buruk: dilepas jadi NULL sehingga tak membatasi apa pun.
type Tahap = (&'static str, &'static str, Option<i64>, bool);

fn stage_for(role: &str, user_id: i64) -> Tahap {
    match role {
        "supervisor" => ("pamong", "Verifikasi Pamong", Some(user_id), true),
        _ => ("final", "Verifikasi Final", None, false), // dewan_guru/admin/ketua
    }
}

/// Tahap yang berlaku bagi `role` pada kelas ber-`mode` (migrasi 62).
///
/// `None` = peran itu memang TIDAK ikut memverifikasi kelas ini — bukan galat,
/// tapi keadaan yang sah dan harus membuat panelnya tak muncul sama sekali
/// ketimbang menampilkan tombol yang pasti ditolak server.
///
/// Perhatikan mode `pamong`: pamong yang MEMFINALKAN, jadi tahapnya "final",
/// bukan "pamong". Kalau dipaksa lewat tahap pamong, absensinya berhenti di
/// pamong_status='approved' dan tak pernah dapat poin — persis kelas yang
/// verifikasinya seolah tak selesai-selesai.
fn stage_untuk_mode(role: &str, mode: &str, user_id: i64) -> Option<Tahap> {
    let pamong = role == "supervisor";
    match (mode, pamong) {
        ("dua_tahap", true) => Some(("pamong", "Verifikasi Pamong", Some(user_id), true)),
        ("dua_tahap", false) => Some(("final", "Verifikasi Final", None, false)),
        // Cukup guru → pamong tak punya peran di sini.
        ("guru", true) => None,
        ("guru", false) => Some(("final", "Verifikasi Final", None, false)),
        // Cukup pamong → pamong memfinalkan, TAPI hanya sesi yang ia tanggung.
        //
        // Dulu baris ini memberi actor=None untuk siapa pun, termasuk pamong.
        // actor=None berarti query verifikasi melepas syarat kepemilikan
        // (`$2 IS NULL OR pamong_id = $2`), jadi pamong kelas A bisa membuka —
        // dan memfinalkan — absensi kelas B. Di mode 'dua_tahap' pamong sudah
        // dibatasi Some(user_id); ketimpangan itulah bugnya.
        //
        // admin/dewan_guru tetap None: pengawasan lintas kelas memang tugasnya.
        ("pamong", true) => Some(("final", "Verifikasi Final", Some(user_id), true)),
        ("pamong", false) => Some(("final", "Verifikasi Final", None, false)),
        _ => Some(stage_for(role, user_id)),
    }
}

pub async fn session_verify(
    pool: &Pool,
    session_id: i64,
    role: &str,
    user_id: i64,
) -> Result<SessionVerifyData> {
    let mode = crate::repository::session_verify_mode(pool, session_id)
        .await?
        .unwrap_or_else(|| "dua_tahap".to_string());
    let Some((stage, stage_label, actor, actor_pamong)) = stage_untuk_mode(role, &mode, user_id)
    else {
        // Peran ini tak ikut memverifikasi kelas tsb → daftar kosong, panel
        // tak dirender.
        return Ok(SessionVerifyData {
            stage: String::new(),
            stage_label: String::new(),
            items: Vec::new(),
        });
    };
    let rows = repo::session_verify_list(pool, session_id, stage, actor, actor_pamong).await?;
    Ok(SessionVerifyData {
        stage: stage.to_string(),
        stage_label: stage_label.to_string(),
        items: rows
            .into_iter()
            .map(|r| SessionVerifyItem {
                att_id: r.id,
                name: r.full_name,
                nis: r.nis.unwrap_or_else(|| "-".into()),
                status: r.status,
            })
            .collect(),
    })
}

/// Proses verifikasi seluruh sesi: setujui semua yang pending KECUALI `reject_ids`
/// (yang ditolak). Loop memakai `decide_pamong`/`decide_verify` yang sudah benar
/// (poin diberikan sekali di tahap final). Return jumlah yang diproses.
pub async fn decide_session(
    pool: &Pool,
    session_id: i64,
    role: &str,
    user_id: i64,
    reject_ids: &[i64],
) -> Result<i64> {
    let mode = crate::repository::session_verify_mode(pool, session_id)
        .await?
        .unwrap_or_else(|| "dua_tahap".to_string());
    let Some((stage, _, actor, actor_pamong)) = stage_untuk_mode(role, &mode, user_id) else {
        bail_user!("Kelas ini tak memerlukan verifikasi dari peran Anda.");
    };
    let rows = repo::session_verify_list(pool, session_id, stage, actor, actor_pamong).await?;

    // Dipisah jadi DUA daftar lalu ditulis sekali masing-masing. Dulu satu
    // panggilan per santri: sesi 200 santri = 200 transaksi dan 400 perjalanan
    // ke database untuk satu tombol, dan bila koneksi putus di tengah, separuh
    // sesi terverifikasi tanpa ada yang tahu di mana berhentinya.
    let (tolak, setuju): (Vec<i64>, Vec<i64>) =
        rows.into_iter().map(|r| r.id).partition(|id| reject_ids.contains(id));

    let mut n = 0i64;
    if stage == "pamong" {
        // Tahap pamong belum punya jalur set-based tersendiri: ia tak memberi
        // poin, jadi biayanya satu UPDATE per baris tanpa insert menyertai.
        for id in tolak.iter().chain(setuju.iter()) {
            let approve = !reject_ids.contains(id);
            if repo::decide_pamong(pool, *id, user_id, approve, actor).await? {
                n += 1;
            }
        }
        return Ok(n);
    }

    if !tolak.is_empty() {
        n += repo::decide_verify_bulk(pool, &tolak, user_id, false, actor, actor_pamong).await?;
    }
    if !setuju.is_empty() {
        n += repo::decide_verify_bulk(pool, &setuju, user_id, true, actor, actor_pamong).await?;
    }
    Ok(n)
}

/// Koreksi status absensi. Hanya guru pengisi / pamong bertugas sesi itu.
///
/// Status yang diizinkan dibatasi ke yang masuk akal dikoreksi manusia —
/// `outside_schedule` sengaja TIDAK termasuk karena itu hasil pembacaan mesin
/// (tap di luar jadwal), bukan penilaian.
pub async fn correct_attendance(
    pool: &Pool,
    att_id: i64,
    new_status: &str,
    actor_id: i64,
) -> Result<()> {
    if !matches!(new_status, "present" | "late" | "absent" | "permit" | "sick") {
        bail_user!("Status koreksi tidak valid.");
    }
    if !repo::correct_attendance(pool, att_id, new_status, actor_id).await? {
        bail_user!(
            "Tidak bisa dikoreksi: statusnya sudah sama, atau Anda bukan guru/pamong \
             yang bertugas di sesi ini."
        );
    }
    Ok(())
}

/// Koreksi absensi SATU sesi untuk BANYAK santri sekaligus.
///
/// Dua hal yang dikerjakan server, bukan klien:
///
/// 1. **Kewenangan diperiksa SEKALI** untuk seluruh permintaan (guru pengisi /
///    pamong sesi, dengan fallback ke wali & pamong kelas bila sesi belum
///    menetapkan petugasnya) — bukan sekali per santri.
///
/// 2. **Hadir vs terlambat ditentukan dari JAM**, bukan dari tombol yang
///    ditekan. Tiap jadwal sudah punya `limit_entery_time`; membiarkan petugas
///    memilih sendiri berarti batas itu bisa dilanggar tanpa sengaja — dan dua
///    santri dengan jam masuk sama bisa berakhir beda status. Kalau jamnya
///    dikosongkan, status yang dipilih petugas dipakai apa adanya.
///
/// Return jumlah baris yang tersimpan.
pub async fn correct_attendance_bulk(
    pool: &Pool,
    session_id: i64,
    items: &[crate::models::KoreksiAbsensi],
    actor_id: i64,
) -> Result<i64> {
    if items.is_empty() {
        bail_user!("Tak ada perubahan untuk disimpan.");
    }
    let Some((schedule_id, session_date, limit_time)) =
        repo::session_for_correction(pool, session_id, actor_id).await?
    else {
        bail_user!("Anda bukan guru/pamong yang bertugas di sesi ini.");
    };

    // Validasi & normalisasi dulu SELURUH baris, baru sekali tulis. Dulu tiap
    // santri satu upsert: satu sesi 200 santri = 200 transaksi dan 200 kali
    // perjalanan ke database untuk satu tombol "simpan".
    //
    // Validasi tetap MENGGAGALKAN SEMUA bila ada satu status tak dikenal —
    // menyimpan sebagian dari koreksi massal meninggalkan sesi setengah benar
    // yang tak bisa dibedakan dari yang belum dikoreksi.
    let mut uids: Vec<i64> = Vec::with_capacity(items.len());
    let mut statuses: Vec<String> = Vec::with_capacity(items.len());
    let mut jams: Vec<Option<chrono::NaiveTime>> = Vec::with_capacity(items.len());

    for it in items {
        if !matches!(
            it.status.as_str(),
            "present" | "late" | "absent" | "permit" | "sick"
        ) {
            bail_user!("Status \"{}\" tidak dikenal.", it.status);
        }
        let jam = parse_jam(&it.jam)?;

        // Jam menentukan hadir/telat; batas jadwal yang jadi acuannya.
        let status = match (jam, limit_time) {
            (Some(j), Some(batas)) if matches!(it.status.as_str(), "present" | "late") => {
                if j > batas {
                    "late"
                } else {
                    "present"
                }
            }
            _ => it.status.as_str(),
        };

        uids.push(it.user_id);
        statuses.push(status.to_string());
        jams.push(jam);
    }

    let n = repo::upsert_session_attendance_bulk(
        pool,
        session_id,
        schedule_id,
        session_date,
        &uids,
        &statuses,
        &jams,
        actor_id,
    )
    .await?;

    Ok(n)
}

/// "HH:MM" → jam. Kosong = None. Format lain ditolak dengan pesan yang jelas
/// ketimbang diam-diam dianggap kosong.
fn parse_jam(s: &str) -> Result<Option<chrono::NaiveTime>> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(None);
    }
    chrono::NaiveTime::parse_from_str(s, "%H:%M")
        .map(Some)
        .map_err(|_| anyhow::anyhow!("Jam \"{s}\" tidak valid (format 24 jam, mis. 05:15)."))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pamong hanya boleh menyentuh sesi yang IA tanggung — di ketiga mode.
    ///
    /// Regresi: di mode 'pamong' baris ini pernah mengembalikan actor `None`,
    /// yang di query verifikasi berarti "tanpa batas kepemilikan". Pamong kelas
    /// A karenanya bisa memfinalkan absensi kelas B, lengkap dengan poinnya.
    #[test]
    fn pamong_selalu_terikat_sesinya() {
        for mode in ["dua_tahap", "pamong"] {
            let (_, _, actor, actor_pamong) =
                stage_untuk_mode("supervisor", mode, 7).expect("pamong ikut verifikasi");
            assert_eq!(actor, Some(7), "mode {mode}: pamong wajib dibatasi id-nya");
            assert!(actor_pamong, "mode {mode}: id itu dibandingkan ke kolom PAMONG");
        }
        // Mode 'guru' → pamong memang tak ikut sama sekali.
        assert!(stage_untuk_mode("supervisor", "guru", 7).is_none());
    }

    /// Pamong memfinalkan di mode 'pamong' (bukan berhenti di tahap pamong,
    /// yang akan membuat absensinya tak pernah berpoin).
    #[test]
    fn mode_pamong_langsung_final() {
        let (stage, _, _, _) = stage_untuk_mode("supervisor", "pamong", 7).unwrap();
        assert_eq!(stage, "final");
        let (stage, _, _, _) = stage_untuk_mode("supervisor", "dua_tahap", 7).unwrap();
        assert_eq!(stage, "pamong");
    }

    /// Pengawas lintas-kelas tetap tanpa batas, dan dibandingkan ke kolom GURU.
    #[test]
    fn dewan_dan_admin_mengawasi_semua() {
        for mode in ["dua_tahap", "guru", "pamong"] {
            let (stage, _, actor, actor_pamong) = stage_untuk_mode("dewan_guru", mode, 9).unwrap();
            assert_eq!(stage, "final", "mode {mode}");
            assert_eq!(actor, None, "mode {mode}: pengawas tak dibatasi");
            assert!(!actor_pamong, "mode {mode}");
        }
    }
}
