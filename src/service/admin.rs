//! service/admin.rs — Halaman "User Control" (admin-only, migrasi 17): daftar
//! user + statistik, aktif/nonaktifkan akun, ganti peran — semua aksi tercatat
//! ke activity_logs.

use anyhow::Result;
use deadpool_postgres::Pool;

use super::fmt::fmt_ago;
use crate::models::{ActivityLogItem, RfidDeviceItem, UserControlData, UserRow};
use crate::repository as repo;

/// Saldo poin awal semester (PRD "Sistem Poin 2.0": 300 poin).
///
/// Nilainya kini tinggal di `models` supaya `repository` juga bisa memakainya
/// tanpa membalik arah lapisan — lihat catatan di sana. Diekspor ulang di sini
/// agar pemanggil lama tak perlu diubah.
pub use crate::models::SEMESTER_START_POINTS;

/// Reset saldo poin semua santri ke 300 (awal semester baru, PRD). Return
/// jumlah santri ter-reset.
pub async fn reset_semester_points(pool: &Pool) -> Result<i64> {
    repo::reset_semester_points(pool, SEMESTER_START_POINTS).await
}

/// api_key acak (32 hex) untuk perangkat RFID baru.
/// SHA-256 hex dari api_key perangkat.
///
/// Hash CEPAT, bukan bcrypt: fungsi ini dipanggil pada SETIAP tap kartu, dan
/// bcrypt yang sengaja lambat (~80 ms) akan membuat mesin absensi tersendat.
///
/// BATAS PERLINDUNGANNYA JUJUR: kunci 16 digit = ~53 bit, jadi bila dump DB
/// bocor, hash-nya bisa dibongkar dengan tenaga GPU dalam hitungan bulan. Yang
/// dicegah di sini adalah kebocoran DB langsung menyerahkan kunci yang SIAP
/// PAKAI. Bila sebuah kunci dicurigai bocor, ganti lewat tombol regenerasi —
/// jauh lebih murah daripada memperpanjang kunci dan menyulitkan pengetikan.
pub fn hash_api_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(key.trim().as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// api_key perangkat RFID — DIGIT SAJA, 16 angka (migrasi 49). Dulu 32 hex;
/// diubah karena kunci ini diketik manual di captive portal firmware ESP8266,
/// dan huruf hex mudah keliru (0/O, b/6). 16 digit ≈ 53 bit: ruang 10^16 masih
/// jauh di luar jangkauan tebak-tebakan lewat jaringan.
///
/// Digit pertama dijaga bukan 0 supaya kunci tak terpotong bila ada firmware /
/// spreadsheet yang memperlakukannya sebagai bilangan.
fn gen_api_key() -> String {
    use rand::RngExt;
    let mut rng = rand::rng();
    let mut key = String::with_capacity(16);
    key.push(char::from_digit(rng.random_range(1..=9), 10).expect("1..9 valid"));
    for _ in 1..16 {
        key.push(char::from_digit(rng.random_range(0..=9), 10).expect("0..9 valid"));
    }
    key
}

/// Validasi kategori perangkat terhadap daftar sah (= CHECK constraint DB).
/// Kosong → 'custom' (perilaku absensi kelas biasa).
fn norm_category(c: &str) -> Result<String> {
    let c = c.trim();
    if c.is_empty() {
        return Ok("custom".to_string());
    }
    if !crate::models::DEVICE_CATEGORIES.iter().any(|(v, _)| *v == c) {
        bail_user!("Kategori perangkat tidak dikenal.");
    }
    Ok(c.to_string())
}

/// Daftar perangkat RFID (ruang) untuk manajemen admin + dropdown jadwal.
pub async fn rfid_devices(pool: &Pool) -> Result<Vec<RfidDeviceItem>> {
    Ok(repo::list_devices(pool)
        .await?
        .into_iter()
        .map(|d| RfidDeviceItem {
            id: d.id,
            device_name: d.device_name,
            serial_number: d.serial_number.unwrap_or_default(),
            location: d.location.unwrap_or_default(),
            api_key: d.api_key,
            category: d.category,
        })
        .collect())
}

/// Buat perangkat RFID. `api_key` kosong → di-generate. Return (id, api_key)
/// — kunci HANYA dikembalikan di sini; setelah ini hanya hash-nya yang tersimpan.
pub async fn create_rfid_device(
    pool: &Pool,
    device_name: &str,
    serial_number: &str,
    location: &str,
    api_key: &str,
    category: &str,
) -> Result<(i64, String)> {
    let name = device_name.trim();
    if name.is_empty() {
        bail_user!("Nama perangkat/ruang wajib diisi.");
    }
    let serial = serial_number.trim();
    let loc = location.trim();
    let key = api_key.trim();
    let key = if key.is_empty() { gen_api_key() } else { key.to_string() };
    let cat = norm_category(category)?;
    // Kembalikan kuncinya: sejak disimpan sebagai hash (migrasi 53), inilah
    // SATU-SATUNYA kesempatan admin melihatnya. Tak dikembalikan = perangkat
    // baru tak bisa dikonfigurasi tanpa langsung menggantinya.
    let id = repo::create_device(
        pool,
        name,
        (!serial.is_empty()).then_some(serial),
        (!loc.is_empty()).then_some(loc),
        &key,
        &cat,
    )
    .await?;
    Ok((id, key))
}

pub async fn update_rfid_device(
    pool: &Pool,
    id: i64,
    device_name: &str,
    serial_number: &str,
    location: &str,
    category: &str,
) -> Result<()> {
    let name = device_name.trim();
    if name.is_empty() {
        bail_user!("Nama perangkat/ruang wajib diisi.");
    }
    let serial = serial_number.trim();
    let loc = location.trim();
    let cat = norm_category(category)?;
    if !repo::update_device(
        pool,
        id,
        name,
        (!serial.is_empty()).then_some(serial),
        (!loc.is_empty()).then_some(loc),
        &cat,
    )
    .await?
    {
        bail_user!("Perangkat tidak ditemukan.");
    }
    Ok(())
}

/// Ganti api_key perangkat (mis. bila bocor) → return api_key baru.
pub async fn regenerate_rfid_key(pool: &Pool, id: i64) -> Result<String> {
    let key = gen_api_key();
    if !repo::set_api_key(pool, id, &key).await? {
        bail_user!("Perangkat tidak ditemukan.");
    }
    Ok(key)
}

pub async fn delete_rfid_device(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_device(pool, id).await? {
        bail_user!("Perangkat tidak ditemukan.");
    }
    Ok(())
}

// `role_label` pindah ke `models` supaya repository & komponen memakai daftar
// yang SAMA. Versi lama di sini tak mengenal "ketua" maupun "santri_finance" —
// keduanya tampil sebagai "Pengguna" di halaman kontrol pengguna.
use crate::models::role_label;

fn action_label(action: &str) -> String {
    match action {
        "user.activate" => "Aktifkan Akun".into(),
        "user.deactivate" => "Nonaktifkan Akun".into(),
        "user.role_change" => "Ganti Peran".into(),
        "user.delete" => "Hapus Akun".into(),
        other => other.into(),
    }
}

/// Payload halaman User Control.
///
/// `can_manage` false (guru/pamong) → daftar pengguna TIDAK diambil sama
/// sekali. Mengunci tombol di UI saja tak cukup: daftar nama, kontak, dan
/// peran seluruh penghuni pondok tetap terkirim ke browser dan bisa dibaca
/// siapa pun yang membuka DevTools. Yang boleh mereka lihat cuma jejak
/// aktivitas, dan itu diambil server fn terpisah.
pub async fn user_control_data(
    pool: &Pool,
    role_filter: Option<&str>,
    can_manage: bool,
) -> Result<UserControlData> {
    if !can_manage {
        return Ok(UserControlData {
            can_manage: false,
            total: 0,
            santri_count: 0,
            staff_count: 0,
            inactive_count: 0,
            users: Vec::new(),
        });
    }
    let (counts, rows) = tokio::join!(
        repo::user_counts(pool),
        repo::list_users(pool, role_filter, 500),
    );
    let (total, santri_count, staff_count, inactive_count) = counts?;
    let users = rows?
        .into_iter()
        .map(|u| {
            let contact = if u.role == "santri" {
                u.nis.unwrap_or_default()
            } else {
                u.email.or(u.username).unwrap_or_default()
            };
            UserRow {
                id: u.id,
                name: u.full_name,
                role_label: role_label(&u.role).into(),
                role: u.role,
                contact,
                is_active: u.is_active,
            }
        })
        .collect();

    Ok(UserControlData { can_manage: true, total, santri_count, staff_count, inactive_count, users })
}

pub async fn recent_activity(pool: &Pool, hari: i32, limit: i64) -> Result<Vec<ActivityLogItem>> {
    Ok(repo::recent_logs(pool, hari, limit)
        .await?
        .into_iter()
        .map(|l| ActivityLogItem {
            actor_name: l.actor_name.unwrap_or_else(|| "Sistem".into()),
            target_name: l.target_name,
            action_label: action_label(&l.action),
            detail: l.detail,
            when_label: fmt_ago(l.created_at),
        })
        .collect())
}

pub async fn toggle_active(pool: &Pool, actor_id: i64, target_id: i64, active: bool) -> Result<()> {
    if actor_id == target_id {
        bail_user!("Tidak bisa mengubah status akun sendiri.");
    }
    if !repo::set_active(pool, target_id, active).await? {
        bail_user!("Pengguna tidak ditemukan.");
    }
    let action = if active { "user.activate" } else { "user.deactivate" };
    let _ = repo::insert_log(pool, actor_id, Some(target_id), action, None).await;
    Ok(())
}

/// Ganti peran seseorang. Return kalimat untuk ditampilkan ke pengelola.
///
/// `actor_role` = peran PEMANGGIL, dan ia menentukan — lihat
/// [`crate::models::can_change_role`]: hanya ketua yang boleh mengangkat maupun
/// mencabut peran ketua.
///
/// ── KETUA ITU SATU, DAN MENUNJUKNYA BERARTI MENYERAHKANNYA ───────────────────
/// Memilih "Ketua" untuk orang lain bukan menambah ketua, melainkan
/// MEMINDAHKAN jabatannya: yang ditunjuk naik, dan ketua lama — yaitu pemanggil
/// sendiri, karena hanya ketua yang boleh menunjuk — otomatis turun jadi
/// `admin`. Satu transaksi, lihat [`repo::transfer_ketua`].
///
/// ── SATU PENGECUALIAN: JABATAN YANG MASIH KOSONG ─────────────────────────────
/// Bila BELUM ADA ketua sama sekali, admin biasa boleh menunjuk yang pertama.
/// Tanpa jalan ini, instalasi baru terkunci selamanya: admin seed bukan ketua,
/// dan aturan "hanya ketua yang menunjuk ketua" berarti tak seorang pun bisa
/// mengangkat siapa pun — satu-satunya jalan keluarnya menulis SQL langsung ke
/// produksi. Pengecualiannya sempit dan menutup dirinya sendiri: begitu ada
/// satu ketua, pintu ini terkunci dan hanya dia yang bisa memindahkannya.
pub async fn change_role(
    pool: &Pool,
    actor_id: i64,
    actor_role: &str,
    target_id: i64,
    new_role: &str,
) -> Result<String> {
    if actor_id == target_id {
        bail_user!("Tidak bisa mengubah peran akun sendiri.");
    }
    // Peran SEKARANG dibaca segar dari DB, bukan dari baris yang kebetulan ada
    // di layar pemanggil: daftar di layar bisa berumur beberapa menit, dan
    // penjagaan yang bersandar pada data basi bisa dilewati hanya dengan
    // membiarkan tab terbuka cukup lama.
    let Some(peran_kini) = repo::role_of_user(pool, target_id).await? else {
        bail_user!("Pengguna tidak ditemukan.");
    };
    if peran_kini == new_role {
        bail_user!("Peran orang ini memang sudah {}.", role_label(new_role));
    }

    // Jabatan kosong → admin boleh mengangkat yang pertama (lihat catatan di
    // atas). Diperiksa hanya bila memang perlu, supaya jalur biasa tak
    // menambah query.
    let boleh = crate::models::can_change_role(actor_role, &peran_kini, new_role)
        || (new_role == "ketua" && repo::jumlah_ketua(pool).await? == 0);
    if !boleh {
        bail_user!(
            "Hanya Ketua yang boleh mengangkat atau mencabut peran Ketua. \
             Mintakan perubahan ini kepada Ketua."
        );
    }

    if new_role == "ketua" {
        return serahkan_ketua(pool, actor_id, target_id).await;
    }

    if !repo::set_role(pool, target_id, new_role).await? {
        bail_user!("Peran tidak valid atau pengguna tidak ditemukan.");
    }
    let detail = format!("Peran baru: {}", role_label(new_role));
    let _ = repo::insert_log(pool, actor_id, Some(target_id), "user.role_change", Some(&detail)).await;
    Ok(format!("Peran diubah menjadi {}.", role_label(new_role)))
}

/// Serahkan jabatan ketua ke `target_id` — dan catat KEDUA sisinya.
///
/// Jejaknya sengaja dua baris, bukan satu: enam bulan dari sekarang, pertanyaan
/// yang muncul bukan "siapa yang diangkat" melainkan "sejak kapan si anu bukan
/// ketua lagi", dan pertanyaan itu hanya terjawab bila penurunannya juga
/// tercatat atas namanya sendiri.
async fn serahkan_ketua(pool: &Pool, actor_id: i64, target_id: i64) -> Result<String> {
    let (ok, diturunkan) = repo::transfer_ketua(pool, target_id).await?;
    if !ok {
        bail_user!("Pengguna tidak ditemukan.");
    }

    let detail_naik = "Peran baru: Ketua (jabatan diserahkan)";
    let _ = repo::insert_log(pool, actor_id, Some(target_id), "user.role_change", Some(detail_naik))
        .await;
    for id in &diturunkan {
        let _ = repo::insert_log(
            pool,
            actor_id,
            Some(*id),
            "user.role_change",
            Some("Peran baru: Admin — jabatan Ketua diserahkan ke orang lain"),
        )
        .await;
    }

    Ok(if diturunkan.contains(&actor_id) {
        "Jabatan Ketua diserahkan. Peran Anda sendiri kini Admin.".to_string()
    } else if diturunkan.is_empty() {
        "Ketua ditunjuk.".to_string()
    } else {
        "Jabatan Ketua dipindahkan; ketua sebelumnya kini Admin.".to_string()
    })
}

/// Hapus satu akun BESERTA seluruh datanya. Return kalimat ringkasan untuk
/// ditampilkan ke pengelola.
///
/// Wewenang KETUA saja, dan diperiksa lagi di sini meski endpoint-nya juga
/// menjaga: ini satu-satunya aksi di aplikasi yang tak bisa dibatalkan, jadi ia
/// tak boleh bersandar pada satu penjaga yang bisa hilang saat endpoint disalin.
///
/// Untuk santri yang selesai mondok, yang benar tetap NONAKTIFKAN — riwayat
/// kehadiran, poin, dan tagihannya masih dirujuk laporan. Penghapusan ini untuk
/// akun yang memang tak boleh ada: salah daftar, duplikat, atau orang yang
/// memintanya.
pub async fn delete_user(
    pool: &Pool,
    actor_id: i64,
    actor_role: &str,
    target_id: i64,
) -> Result<String> {
    if actor_role != "ketua" {
        bail_user!("Hanya Ketua yang boleh menghapus akun beserta seluruh datanya.");
    }
    if actor_id == target_id {
        bail_user!("Tidak bisa menghapus akun sendiri.");
    }
    let Some(hasil) = repo::delete_user_cascade(pool, target_id).await? else {
        bail_user!("Pengguna tidak ditemukan.");
    };

    let total: i64 = hasil.baris.iter().map(|(_, n)| *n).sum();
    let rincian = hasil
        .baris
        .iter()
        .map(|(t, n)| format!("{t}: {n}"))
        .collect::<Vec<_>>()
        .join(", ");

    // Jejaknya ditulis TANPA `target_user_id` — barisnya sudah tak ada, dan
    // kolom itu ON DELETE SET NULL. Nama & peran karena itu ikut dititipkan ke
    // `detail`: tanpa keduanya, log hanya bercerita bahwa "seseorang" dihapus.
    let detail = format!(
        "Hapus akun: {} ({}) — {total} baris data terkait{}",
        hasil.full_name,
        role_label(&hasil.role),
        if rincian.is_empty() { String::new() } else { format!(" [{rincian}]") }
    );
    let _ = repo::insert_log(pool, actor_id, None, "user.delete", Some(&detail)).await;

    Ok(format!(
        "Akun {} dihapus beserta {total} baris data terkait.",
        hasil.full_name
    ))
}
