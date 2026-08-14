//! service/santri.rs — Logika halaman santri: riwayat kehadiran, izin, profil.

use anyhow::Result;
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use deadpool_postgres::Pool;

use super::fmt::{fmt_dt_full, fmt_month, wib};
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

/// Awal "semester berjalan". Bila admin sudah mendefinisikan semester yang
/// mencakup HARI INI (migrasi 40) → pakai tanggal mulainya sebagai acuan
/// (otomatis dari tanggal, tanpa aktivasi manual); jika tidak → fallback ke
/// perhitungan otomatis `semester_start()` (Jul=Ganjil / Jan=Genap).
pub(crate) async fn current_semester(pool: &Pool) -> Result<(chrono::DateTime<Utc>, String)> {
    let today = Utc::now().with_timezone(&wib()).date_naive();
    if let Ok(Some(s)) = repo::current_semester(pool, today).await {
        let start_utc = wib()
            .from_local_datetime(&s.start_date.and_hms_opt(0, 0, 0).expect("00:00 valid"))
            .single()
            .map(|d| d.with_timezone(&Utc))
            .unwrap_or_else(|| Utc::now());
        return Ok((start_utc, super::semester::semester_label(&s.kind, s.year)));
    }
    Ok(semester_start())
}

pub(crate) fn status_display(status: &str) -> (&'static str, &'static str) {
    match status {
        "present" => ("HADIR", "present"),
        "late" => ("TERLAMBAT", "late"),
        // Warna reuse "late" (oranye) — visual "irregular"; label dibedakan.
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
            // Angka yang SUDAH tercatat menang atas aturan umum. `point_rule`
            // tinggal dipakai untuk KATA-nya ("Kedisiplinan", "Pelanggaran"),
            // dan sebagai perkiraan bagi baris yang belum diverifikasi —
            // barisan itu memang belum punya catatan poin, dan menampilkan 0
            // di sana akan terbaca sebagai "bebas", padahal belum dinilai.
            let (perkiraan, note, _) = point_rule(&r.status);
            let points = r.points.unwrap_or(perkiraan);
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

    let (stats, home, detected, permits, aktif) = tokio::join!(
        repo::semester_stats(pool, user_id, since),
        repo::user_home(pool, user_id),
        repo::latest_scan_today(pool, user_id, today),
        repo::list_my_permits(pool, user_id, 5),
        super::permits::izin_aktif_saya(pool, user_id),
    );

    let (hadir, _izin, alpa, total) = stats?;
    let pct = if total > 0 { ((hadir * 100) / total) as i32 } else { 0 };
    let points = home?.map(|h| h.points).unwrap_or(0);

    let detected = detected?.map(|(title, ts, _status)| {
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
            let (status_label, status_kind) = permit_stage(&p.guru_status);
            PermitItem {
                id: p.id,
                diajukan_oleh: if p.oleh_ortu {
                    format!("Diajukan orang tua — {}", p.requester_name)
                } else {
                    String::new()
                },
                kind_label: permit_kind_label(&p.kind).into(),
                range_label: super::fmt::fmt_rentang(p.mulai, p.selesai),
                class_label: p.class_name.unwrap_or_default(),
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
        // Gagalnya spanduk TIDAK boleh menggagalkan seluruh halaman izin —
        // yang utama di sini tetap form pengajuannya.
        aktif: aktif.unwrap_or(None),
        permits,
    })
}

/// Ajukan izin baru (validasi di sini — jangan percaya klien). `requested_by`
/// = santri sendiri ATAU orang tua yang mengajukan atas nama anak (lihat
/// `service::parent::submit_child_permit`).
///
/// Migrasi 46: pengajuan DIPECAH per wali kelas yang kelasnya dilewati selama
/// rentang izin (lihat `service::permits::split_permit_per_wali`). Satu ajuan
/// bisa menghasilkan beberapa baris izin yang jalan sendiri-sendiri.
#[allow(clippy::too_many_arguments)]
/// Validasi & normalisasi isi pengajuan izin.
///
/// DIPAKAI BERSAMA oleh pengajuan baru dan penyuntingan (service::permits::
/// update_permit). Aturan yang ditulis dua kali cepat atau lambat berbeda —
/// dan yang berbeda di sini berarti izin bisa disunting jadi bentuk yang tak
/// akan pernah lolos bila diajukan dari awal.
///
/// Return `(kind, start, end, jam, reason)` yang sudah bersih.
/// Validasi pengajuan izin → satu RENTANG WAKTU.
///
/// Masukan dari layar tetap empat kotak (tanggal mulai, tanggal selesai, jam
/// keluar, jam pulang) karena itulah cara orang mengisinya. Yang keluar dari
/// sini SATU rentang — `mulai` sampai `selesai` — dan itulah acuan tunggal
/// berapa kelas yang terlewat (migrasi 86).
///
/// Jam dikosongkan = IZIN SEHARI PENUH → 00:00 sampai 23:59:59. Bukan keadaan
/// khusus yang perlu diingat pembaca berikutnya, cuma rentang selebar satu hari.
pub(crate) fn validasi_izin<'a>(
    kind: &'a str,
    start: &str,
    end: &str,
    jam_mulai: &str,
    jam_selesai: &str,
    reason: &str,
) -> Result<(&'a str, chrono::NaiveDateTime, chrono::NaiveDateTime, String)> {
    if !matches!(kind, "sick" | "leave" | "keperluan") {
        bail_user!("Jenis izin tidak valid.");
    }
    let Ok(start_date) = NaiveDate::parse_from_str(start.trim(), "%Y-%m-%d") else {
        bail_user!("Tanggal mulai wajib diisi.");
    };
    let end_date = match end.trim() {
        "" => start_date,
        s => match NaiveDate::parse_from_str(s, "%Y-%m-%d") {
            Ok(d) if d >= start_date => d,
            Ok(_) => bail_user!("Tanggal selesai tidak boleh sebelum tanggal mulai."),
            Err(_) => bail_user!("Tanggal selesai tidak valid."),
        },
    };
    // Keduanya diisi atau keduanya kosong: satu ujung saja tak membentuk
    // rentang, dan menebak ujung yang lain berarti mengarang izin.
    let (jm, js) = match (jam_mulai.trim(), jam_selesai.trim()) {
        ("", "") => (None, None),
        (a, b) if a.is_empty() || b.is_empty() => {
            bail_user!("Isi jam keluar DAN jam pulang, atau kosongkan keduanya untuk izin sehari penuh.")
        }
        (a, b) => {
            let (Ok(ja), Ok(jb)) = (
                chrono::NaiveTime::parse_from_str(a, "%H:%M"),
                chrono::NaiveTime::parse_from_str(b, "%H:%M"),
            ) else {
                bail_user!("Jam izin tidak valid (format 24 jam, mis. 09:00).");
            };
            (Some(ja), Some(jb))
        }
    };
    let akhir_hari = chrono::NaiveTime::from_hms_opt(23, 59, 59).expect("23:59:59 valid");
    let mulai = start_date.and_time(jm.unwrap_or(chrono::NaiveTime::MIN));
    let selesai = end_date.and_time(js.unwrap_or(akhir_hari));
    // Urutan diperiksa pada RENTANGNYA, bukan pada jamnya. Pulang pukul 08:00
    // setelah keluar pukul 14:00 dua hari sebelumnya jelas sah — aturan lama
    // yang cuma membandingkan jam menolaknya.
    if selesai <= mulai {
        bail_user!("Waktu pulang harus setelah waktu keluar.");
    }
    let reason = reason.trim();
    if reason.chars().count() < 5 {
        bail_user!("Tuliskan alasan izin (minimal 5 karakter).");
    }
    Ok((kind, mulai, selesai, reason.chars().take(500).collect()))
}

pub async fn submit_permit(
    pool: &Pool,
    user_id: i64,
    requested_by: i64,
    kind: &str,
    start: &str,
    end: &str,
    // "HH:MM" keduanya, atau keduanya kosong = izin sehari penuh (migrasi 66).
    jam_mulai: &str,
    jam_selesai: &str,
    reason: &str,
) -> Result<Vec<super::permits::PermitSplit>> {
    let (kind, mulai, selesai, reason) =
        validasi_izin(kind, start, end, jam_mulai, jam_selesai, reason)?;

    super::permits::split_permit_per_wali(pool, user_id, requested_by, kind, mulai, selesai, &reason)
    .await
}

/// Kelas apa saja yang akan terlewat bila izin ini diajukan — dihitung SEBELUM
/// pengajuan dibuat.
///
/// Memakai jalur yang sama persis dengan `submit_permit` (repo::affected_classes
/// + saringan recurrence), jadi angka yang dilihat santri tak bisa berbeda dari
/// yang nanti benar-benar terjadi. Input tak lengkap/tak valid → pratinjau
/// kosong, bukan galat: ini dipanggil sambil orang mengetik.
pub async fn pratinjau_izin(
    pool: &Pool,
    user_id: i64,
    start: &str,
    end: &str,
    jam_mulai: &str,
    jam_selesai: &str,
) -> Result<crate::models::PratinjauIzin> {
    let kosong = crate::models::PratinjauIzin::default();
    // Dipanggil SAMBIL ORANG MENGETIK: masukan setengah jadi menghasilkan
    // pratinjau kosong, bukan galat. Aturan rentangnya sendiri sama persis
    // dengan pengajuan sungguhan — dipinjam dari `validasi_izin` supaya angka
    // yang dilihat santri mustahil berbeda dari yang nanti terjadi.
    let Ok((_, mulai, selesai, _)) =
        validasi_izin("leave", start, end, jam_mulai, jam_selesai, "pratinjau")
    else {
        return Ok(kosong);
    };

    let affected =
        repo::affected_classes(pool, user_id, mulai.date(), selesai.date()).await?;

    // Satu kelas bisa punya beberapa jadwal; sesinya dijumlahkan, kelasnya
    // tetap satu baris. Urutan kemunculan pertama dipertahankan supaya daftar
    // tak berubah-ubah tiap kali diketik.
    let mut urut: Vec<i64> = Vec::new();
    let mut per_kelas: std::collections::HashMap<i64, crate::models::PratinjauKelas> =
        std::collections::HashMap::new();
    let mut wali: std::collections::HashSet<i64> = std::collections::HashSet::new();
    let mut total = 0i64;

    for c in &affected {
        // Aturan yang SAMA dengan pengajuan sungguhan (`tanggal_izin`) — angka
        // di pratinjau tak boleh berbeda dari yang nanti benar-benar terjadi.
        let n = super::permits::tanggal_izin(c, mulai, selesai).len() as i64;
        if n == 0 {
            continue;
        }
        total += n;
        if let Some(w) = c.wali_kelas_id {
            wali.insert(w);
        }
        let e = per_kelas.entry(c.class_id).or_insert_with(|| {
            urut.push(c.class_id);
            crate::models::PratinjauKelas {
                nama: c.class_name.clone(),
                kategori: String::new(),
                wali: c.wali_name.clone().unwrap_or_default(),
                sesi: 0,
            }
        });
        e.sesi += n;
    }

    let kategori = repo::kategori_kelas(pool, &urut).await.unwrap_or_default();
    let kelas = urut
        .into_iter()
        .filter_map(|id| {
            per_kelas.remove(&id).map(|mut k| {
                if let Some(cat) = kategori.get(&id) {
                    k.kategori = crate::models::kategori_label(cat).to_string();
                }
                k
            })
        })
        .collect();

    Ok(crate::models::PratinjauIzin {
        kelas,
        total_sesi: total,
        total_wali: wali.len() as i64,
    })
}

/// Data profil pengguna.
pub async fn profil(pool: &Pool, user_id: i64) -> Result<ProfilData> {
    let Some(p) = repo::profil_row(pool, user_id).await? else {
        bail_user!("unauth");
    };
    let role_label = match p.role.as_str() {
        "admin" => "ADMIN",
        "teacher" | "dewan_guru" => "DEWAN GURU",
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
        entry_year: p.entry_year,
        ipk_history,
    })
}

/// Ubah profil mahasiswa (kampus/jurusan/gender/tahun masuk). Kosong → NULL.
pub async fn update_profile_extra(
    pool: &Pool,
    user_id: i64,
    campus: &str,
    major: &str,
    gender: &str,
    entry_year: &str,
) -> Result<()> {
    let opt = |s: &str| -> Option<String> {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    let g = match gender.trim() {
        "L" | "P" => Some(gender.trim().to_string()),
        _ => None,
    };
    // Tahun masuk PPM (bukan tahun masuk kuliah — lihat migrasi 47): kosong →
    // NULL; ada isi → wajib tahun 4-digit yang masuk akal.
    let ey = entry_year.trim();
    let year: Option<i16> = if ey.is_empty() {
        None
    } else {
        let y: i16 = ey
            .parse()
            .map_err(|_| anyhow::anyhow!("Tahun masuk PPM harus berupa angka (mis. 2024)."))?;
        if !(1990..=2100).contains(&y) {
            bail_user!("Tahun masuk PPM tidak masuk akal (1990–2100).");
        }
        Some(y)
    };
    repo::update_profile_extra(
        pool,
        user_id,
        opt(campus).as_deref(),
        opt(major).as_deref(),
        g.as_deref(),
        year,
    )
    .await
}

/// Ubah kontak (email + alamat) user yang login — semua peran. Kosong → NULL
/// (email kosong disimpan NULL agar tak bentrok UNIQUE antar user tanpa email).
pub async fn update_contact(pool: &Pool, user_id: i64, email: &str, address: &str) -> Result<()> {
    let email = email.trim();
    if !email.is_empty() && (!email.contains('@') || !email.contains('.') || email.len() < 5) {
        bail_user!("Format email tidak valid.");
    }
    let opt = |s: &str| -> Option<String> {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    repo::update_contact(pool, user_id, opt(email).as_deref(), opt(address).as_deref()).await
}

/// Tambah entri IPK santri. `ipk` teks (0.00–4.00).
pub async fn add_ipk(pool: &Pool, user_id: i64, semester: &str, ipk: &str) -> Result<i64> {
    let semester = semester.trim();
    if semester.is_empty() {
        bail_user!("Semester wajib diisi (mis. 2024/2025 Ganjil).");
    }
    let val: f64 = ipk
        .trim()
        .replace(',', ".")
        .parse()
        .map_err(|_| anyhow::anyhow!("IPK harus berupa angka (mis. 3.75)."))?;
    if !(0.0..=4.0).contains(&val) {
        bail_user!("IPK harus di antara 0.00 sampai 4.00.");
    }
    repo::add_ipk(pool, user_id, semester, val).await
}

pub async fn delete_ipk(pool: &Pool, user_id: i64, id: i64) -> Result<()> {
    if !repo::delete_ipk(pool, user_id, id).await? {
        bail_user!("Entri IPK tidak ditemukan.");
    }
    Ok(())
}

#[cfg(test)]
mod tests_izin {
    use super::validasi_izin;

    fn ok(kind: &str, s: &str, e: &str, jm: &str, js: &str) -> (String, String) {
        let (_, mulai, selesai) = match validasi_izin(kind, s, e, jm, js, "sakit demam") {
            Ok((k, m, sl, _)) => (k, m, sl),
            Err(err) => panic!("harusnya sah: {err}"),
        };
        (mulai.to_string(), selesai.to_string())
    }
    fn gagal(kind: &str, s: &str, e: &str, jm: &str, js: &str, alasan: &str) -> String {
        validasi_izin(kind, s, e, jm, js, alasan)
            .err()
            .expect("harusnya ditolak")
            .to_string()
    }

    /// Tanggal + jam dirakit jadi SATU rentang — inilah acuan kelas terlewat.
    #[test]
    fn tanggal_dan_jam_jadi_satu_rentang() {
        let (m, s) = ok("leave", "2026-08-07", "2026-08-09", "14:00", "08:30");
        assert_eq!(m, "2026-08-07 14:00:00");
        assert_eq!(s, "2026-08-09 08:30:00");
    }

    /// Jam kosong = sehari penuh: 00:00 → 23:59:59, bukan NULL.
    #[test]
    fn jam_kosong_jadi_sehari_penuh() {
        let (m, s) = ok("sick", "2026-08-07", "", "", "");
        assert_eq!(m, "2026-08-07 00:00:00");
        assert_eq!(s, "2026-08-07 23:59:59");
    }

    /// Tanggal selesai kosong = pulang di hari yang sama.
    #[test]
    fn tanggal_selesai_kosong_berarti_hari_yang_sama() {
        let (m, s) = ok("keperluan", "2026-08-07", "", "09:00", "12:00");
        assert_eq!(m, "2026-08-07 09:00:00");
        assert_eq!(s, "2026-08-07 12:00:00");
    }

    /// REGRESI: urutan diperiksa pada RENTANGNYA, bukan pada jamnya. Keluar
    /// Jumat 14:00 dan kembali Minggu 08:00 jelas sah — aturan lama yang cuma
    /// membandingkan 08:00 < 14:00 menolaknya.
    #[test]
    fn pulang_pagi_di_hari_lain_tetap_sah() {
        let (m, s) = ok("leave", "2026-08-07", "2026-08-09", "14:00", "08:00");
        assert_eq!(m, "2026-08-07 14:00:00");
        assert_eq!(s, "2026-08-09 08:00:00");
    }

    /// Rentang nol/terbalik di hari yang SAMA tetap ditolak.
    #[test]
    fn rentang_terbalik_ditolak() {
        for (jm, js) in [("14:00", "09:00"), ("09:00", "09:00")] {
            let e = gagal("leave", "2026-08-07", "2026-08-07", jm, js, "keperluan keluarga");
            assert!(e.contains("setelah waktu keluar"), "pesan: {e}");
        }
        let e = gagal("leave", "2026-08-09", "2026-08-07", "", "", "keperluan keluarga");
        assert!(e.contains("sebelum tanggal mulai"), "pesan: {e}");
    }

    /// Satu ujung jam saja tak membentuk rentang — menebak ujung lainnya
    /// berarti mengarang izin atas nama santri.
    #[test]
    fn jam_setengah_terisi_ditolak() {
        for (jm, js) in [("14:00", ""), ("", "08:00")] {
            let e = gagal("leave", "2026-08-07", "", jm, js, "keperluan keluarga");
            assert!(e.contains("kosongkan keduanya"), "pesan: {e}");
        }
    }

    #[test]
    fn jenis_dan_alasan_divalidasi() {
        assert!(gagal("bolos", "2026-08-07", "", "", "", "alasan cukup").contains("tidak valid"));
        assert!(gagal("sick", "2026-08-07", "", "", "", "flu").contains("minimal 5 karakter"));
        assert!(gagal("sick", "", "", "", "", "alasan cukup").contains("Tanggal mulai"));
    }
}
