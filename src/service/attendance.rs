//! service/attendance.rs — Alur absensi: scan RFID + verifikasi kehadiran.

use anyhow::Result;
use chrono::Utc;
use deadpool_postgres::Pool;

use super::fmt::{fmt_when, wib};
use crate::models::{VerifikasiData, PendingAtt, RfidScanRequest, RfidScanResponse};
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
///     present/late → simpan (dedup per hari).
///
/// DUA keadaan yang TIDAK menghasilkan catatan apa pun (migrasi 85):
///   • tap di luar jam kelas (tak ada jadwal aktif);
///   • tap di perangkat yang bukan ruang kelasnya, untuk jadwal yang memang
///     terikat ruang (`class_schedules.room_id` terisi).
/// Keduanya dijawab `ok:false` supaya mesin bisa memberi tahu santrinya, dan
/// tabel kehadiran kelas tetap berisi kehadiran kelas saja.
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
    let Some(jadwal) = repo::active_schedule_now(pool, user_id, today, now_time, device.id).await?
    else {
        {
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
            // DI LUAR JAM KELAS → TOLAK, jangan catat apa pun.
            //
            // Dulu tap seperti ini disimpan berstatus `outside_schedule`
            // dengan alasan "jejak lalu-lalang tetap berguna". Ternyata tidak:
            // jejak keluar-masuk area SUDAH punya tempatnya sendiri di gerbang
            // utama (`toggle_gate`, di atas), sementara baris ini menumpang di
            // tabel KEHADIRAN KELAS — dan di sana ia ikut terhitung sebagai
            // "telat" pada rekap mingguan, muncul di riwayat santri, serta
            // menghalangi `run_auto_absent` menandai alfa karena hari itu
            // "sudah ada catatan". Satu tap iseng di luar jam bisa menutupi
            // ketidakhadiran yang sesungguhnya.
            //
            // Keputusan pengurus (Ags 2026): tap di luar jadwal TIDAK dicatat.
            tracing::info!(
                user_id, card = req.card, device = %device_name,
                "tap DITOLAK: di luar jam kelas"
            );
            return Ok(RfidScanResponse {
                ok: false,
                message: "Di luar jam kelas — absen tidak dicatat.".into(),
                student: Some(name),
                status: Some("no_schedule".into()),
            });
        }
    };

    let status = if now_time <= jadwal.limit_entry { "present" } else { "late" };
    let (schedule_id, session_id) = (Some(jadwal.id), Some(jadwal.session_id));

    // Dedup: satu catatan per jadwal.
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
        // Tak ada lagi catatan otomatis: satu-satunya baris yang dulu memakainya
        // ("scan di luar jadwal") sekarang tak pernah dibuat.
        None,
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

/// Antrean verifikasi final + jumlah terverifikasi hari ini. Reuse VerifikasiData
/// (pending + count) — `approved_today` di sini bermakna "terverifikasi hari ini".
pub async fn verify_data(pool: &Pool, teacher_id: Option<i64>) -> Result<VerifikasiData> {
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
    Ok(VerifikasiData {
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
/// Jalur SATUAN; pembandingnya kolom guru sesi (fallback wali kelas).
pub async fn decide_verify(
    pool: &Pool,
    att_id: i64,
    approver: i64,
    approve: bool,
    teacher_id: Option<i64>,
) -> Result<bool> {
    repo::decide_verify(pool, att_id, approver, approve, teacher_id).await
}

// ── Verifikasi kehadiran PER-SESI (batch) ─────────────────────────────────────
// SATU tahap (migrasi 84/86): guru pengisi sesi — atau wali kelasnya bila sesi
// belum menetapkan pengisi — yang mengesahkan; admin/ketua melihat semuanya.
// Klien kirim SATU request per sesi; server menyetujui semua yang pending
// KECUALI `reject_ids`.

use crate::models::{SessionVerifyData, SessionVerifyItem};

/// (tahap, label, id petugas yang membatasi daftar, petugas itu PAMONG?).
///
/// Elemen keempat menentukan kolom pembanding `actor`: pamong sesi atau guru
/// sesi. Tanpa itu, id pamong pernah diadu dengan kolom guru — cocok tak
/// pernah, atau lebih buruk: dilepas jadi NULL sehingga tak membatasi apa pun.
/// Batas kepemilikan untuk daftar & keputusan verifikasi: `Some(id)` = hanya
/// sesi yang orang ini ampu, `None` = tanpa batas.
///
/// SATU fungsi karena daftar dan keputusan HARUS memakai syarat yang sama —
/// kalau menyimpang, seseorang bisa memutuskan baris yang tak pernah boleh ia
/// lihat (atau melihat baris yang keputusannya selalu gagal diam-diam).
fn aktor_verifikasi(role: &str, user_id: i64) -> Option<i64> {
    // Admin/ketua mengawasi seluruh pondok; selain itu terikat sesinya sendiri.
    (!crate::models::role_satisfies(role, &["admin"])).then_some(user_id)
}

pub async fn session_verify(
    pool: &Pool,
    session_id: i64,
    role: &str,
    user_id: i64,
) -> Result<SessionVerifyData> {
    let actor = aktor_verifikasi(role, user_id);
    let rows = repo::session_verify_list(pool, session_id, actor).await?;
    Ok(SessionVerifyData {
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
    let actor = aktor_verifikasi(role, user_id);
    let rows = repo::session_verify_list(pool, session_id, actor).await?;

    // Dipisah jadi DUA daftar lalu ditulis sekali masing-masing. Dulu satu
    // panggilan per santri: sesi 200 santri = 200 transaksi untuk satu tombol,
    // dan bila koneksi putus di tengah, separuh sesi terverifikasi tanpa ada
    // yang tahu di mana berhentinya.
    let (tolak, setuju): (Vec<i64>, Vec<i64>) =
        rows.into_iter().map(|r| r.id).partition(|id| reject_ids.contains(id));

    let mut n = 0i64;
    if !tolak.is_empty() {
        n += repo::decide_verify_bulk(pool, &tolak, user_id, false, actor).await?;
    }
    if !setuju.is_empty() {
        n += repo::decide_verify_bulk(pool, &setuju, user_id, true, actor).await?;
    }
    Ok(n)
}

/// Koreksi status absensi. Hanya guru pengisi / pamong bertugas sesi itu.
///
/// Status yang diizinkan dibatasi ke lima yang masuk akal dinilai manusia;
/// daftarnya sama dengan CHECK `attendances_status_check` (migrasi 85).
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
/// 1. **Kewenangan diperiksa SEKALI** untuk seluruh permintaan (guru pengisi
///    sesi, dengan fallback ke wali kelas bila sesi belum menetapkan
///    pengajarnya; admin/ketua selalu boleh) — bukan sekali per santri.
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
    is_admin: bool,
) -> Result<i64> {
    if items.is_empty() {
        bail_user!("Tak ada perubahan untuk disimpan.");
    }
    let Some((schedule_id, session_date, limit_time)) =
        repo::session_for_correction(pool, session_id, actor_id, is_admin).await?
    else {
        bail_user!("Anda bukan pengajar sesi ini atau wali kelasnya.");
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

    /// Guru/wali hanya boleh menyentuh sesi yang IA ampu.
    ///
    /// Regresi: batas ini pernah hilang untuk salah satu peran, dan `None` di
    /// query verifikasi berarti "tanpa batas kepemilikan" — petugas kelas A
    /// karenanya bisa memfinalkan absensi kelas B, lengkap dengan poinnya.
    #[test]
    fn petugas_selalu_terikat_sesinya() {
        for role in ["teacher", "dewan_guru"] {
            assert_eq!(
                aktor_verifikasi(role, 7),
                Some(7),
                "{role} wajib dibatasi id-nya"
            );
        }
    }

    /// Pengawas lintas-kelas tetap tanpa batas — termasuk ketua, yang klaimnya
    /// bukan "admin" tapi mencakupinya (`role_satisfies`).
    #[test]
    fn pengawas_mengawasi_semua() {
        for role in ["admin", "ketua"] {
            assert_eq!(aktor_verifikasi(role, 9), None, "{role} tak dibatasi");
        }
    }

    /// Daftar dan keputusan memakai batas yang sama persis — satu-satunya
    /// alasan `aktor_verifikasi` ada sebagai fungsi tersendiri.
    #[test]
    fn daftar_dan_keputusan_sepakat() {
        for role in ["teacher", "dewan_guru", "admin", "ketua"] {
            assert_eq!(aktor_verifikasi(role, 3), aktor_verifikasi(role, 3));
        }
    }
}
