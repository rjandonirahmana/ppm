//! service/registration.rs — Registrasi via link undangan (admin/pamong/dewan
//! guru) + OTP WhatsApp (WAHA), pola sama e-ticketing service/auth.rs
//! (initiate_register/verify_register/generate_random_password/normalize_phone)
//! disederhanakan: JSON (bukan protobuf) utk payload Redis, tanpa proto build
//! pipeline.
//!
//! Alur:
//!   1) Staf buat link (`create_invite`) → Redis `reg-invite:{token}` = peran,
//!      TTL 24 jam.
//!   2) Publik buka link, isi nama+HP (`initiate_register`) → validasi link →
//!      generate password+OTP → Redis `reg:ppm:{phone}` = JSON PendingRegistration,
//!      TTL 10 menit → kirim WA (OTP + password, SATU pesan).
//!   3) Publik masukkan OTP (`verify_register`) → cocok → hapus KEDUA key Redis
//!      (link + pending) → INSERT user (baru di sini Postgres tersentuh) → JWT.
//!
//! TIDAK ADA baris `users` sampai OTP berhasil — selama jendela 10 menit,
//! seluruh data pendaftar cuma hidup di Redis.

use anyhow::Result;
use deadpool_postgres::Pool;
use rand::RngExt;
use redis::{aio::ConnectionManager, AsyncCommands};
use serde::{Deserialize, Serialize};

use crate::config::WahaConfig;
use crate::repository as repo;

/// Peran yang boleh diundang mendaftar sendiri — TIDAK termasuk admin (sama
/// aturan e-ticketing: "Admin accounts cannot self-register").
// 'teacher' dihapus (digabung ke dewan_guru, migrasi 36). Peran finance baru
// (ketua, santri_finance) TIDAK di sini — dibuat admin lewat kontrol pengguna,
// bukan via link undangan publik.
pub const INVITABLE_ROLES: &[&str] = &["dewan_guru", "santri", "parent", "penjaga"];

// Kebijakan siapa-boleh-mengundang-siapa ada di models::can_invite —
// SENGAJA di models, bukan di sini, karena dropdown peran di frontend (WASM)
// perlu menyaring pilihan yang sama. Satu sumber kebenaran untuk dua target.

fn invite_key(token: &str) -> String {
    format!("reg-invite:{token}")
}

/// Counter sisa kuota link (multi-pakai). Ada = link berkuota; tak ada = legacy
/// sekali-pakai.
fn uses_key(token: &str) -> String {
    format!("reg-invite:{token}:uses")
}

fn pending_key(phone: &str) -> String {
    format!("reg:ppm:{phone}")
}

#[derive(Serialize, Deserialize)]
struct PendingRegistration {
    name: String,
    phone: String,
    role: String,
    password: String,
    otp: String,
    // Profil mahasiswa — hanya terisi untuk peran santri (migrasi 47).
    // Default serde agar payload lama di Redis (dari sebelum deploy) tetap
    // bisa di-parse selama 10 menit masa transisi, bukan bikin OTP gagal.
    #[serde(default)]
    gender: String,
    #[serde(default)]
    campus: String,
    #[serde(default)]
    major: String,
    #[serde(default)]
    entry_year: Option<i16>,
    /// Percobaan OTP yang gagal. OTP hanya 6 digit — tanpa batas ini,
    /// 1.000.000 kombinasi bisa ditebak habis lewat jaringan dalam jendela
    /// 10 menit karena tiap percobaan cuma satu GET Redis.
    #[serde(default)]
    otp_attempts: u8,
}

/// Percobaan OTP maksimum sebelum pendaftaran harus diulang dari awal.
const MAX_OTP_ATTEMPTS: u8 = 5;

/// Profil mahasiswa yang diminta saat registrasi santri. Kosong untuk peran
/// lain (guru, pamong, orang tua) — mereka tak punya data ini.
#[derive(Default)]
pub struct StudentProfile {
    /// "L" | "P".
    pub gender: String,
    pub campus: String,
    pub major: String,
    /// Tahun masuk PPM (bukan tahun masuk kuliah) — lihat migrasi 47.
    pub entry_year: String,
}

/// Validasi profil mahasiswa. Dipanggil hanya bila peran undangan = santri;
/// keempat isian WAJIB (keputusan produk: santri tanpa data ini menyulitkan
/// pendataan angkatan & pelaporan).
fn validate_student_profile(p: &StudentProfile) -> Result<(String, String, String, i16)> {
    let gender = p.gender.trim();
    if !matches!(gender, "L" | "P") {
        bail_user!("Pilih jenis kelamin (laki-laki atau perempuan).");
    }
    let campus = p.campus.trim();
    if campus.chars().count() < 2 {
        bail_user!("Nama kampus wajib diisi.");
    }
    let major = p.major.trim();
    if major.chars().count() < 2 {
        bail_user!("Jurusan wajib diisi.");
    }
    let ey = p.entry_year.trim();
    if ey.is_empty() {
        bail_user!("Tahun masuk PPM wajib diisi.");
    }
    let year: i16 = ey
        .parse()
        .map_err(|_| anyhow::anyhow!("Tahun masuk PPM harus berupa angka (mis. 2024)."))?;
    if !(1990..=2100).contains(&year) {
        bail_user!("Tahun masuk PPM tidak masuk akal (1990–2100).");
    }
    // Potong sesuai lebar kolom (VARCHAR(150)) agar tak ditolak DB di akhir alur.
    let cut = |s: &str| -> String { s.chars().take(150).collect() };
    Ok((gender.to_string(), cut(campus), cut(major), year))
}

/// Buat link undangan baru (admin/pamong/dewan guru) — token acak (16 byte,
/// hex) → Redis `reg-invite:{token}` = peran + counter kuota `:uses` = max_uses.
/// `max_uses` (1..=1000) = berapa orang boleh pakai token SAMA (mis. 100 santri
/// sekali intake); `ttl_days` (1..=30) = masa berlaku. Return token mentah
/// (pemanggil merangkai URL `/register?key={token}`).
pub async fn create_invite(
    redis: &mut ConnectionManager,
    by_role: &str,
    role: &str,
    max_uses: i64,
    ttl_days: i64,
) -> Result<String> {
    if !INVITABLE_ROLES.contains(&role) {
        bail_user!("Peran tidak valid untuk registrasi mandiri.");
    }
    if !crate::models::can_invite(by_role, role) {
        bail_user!("Hanya admin yang boleh membuat undangan untuk peran staf.");
    }
    let max_uses = max_uses.clamp(1, 1000);
    let ttl_secs = (ttl_days.clamp(1, 30) as u64) * 86400;
    let mut bytes = [0u8; 16];
    rand::rng().fill(&mut bytes);
    let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();

    let _: () = redis
        .set_ex(invite_key(&token), role, ttl_secs)
        .await
        .map_err(|e| anyhow::anyhow!("Redis SET gagal: {e}"))?;
    let _: () = redis
        .set_ex(uses_key(&token), max_uses, ttl_secs)
        .await
        .map_err(|e| anyhow::anyhow!("Redis SET gagal: {e}"))?;
    Ok(token)
}

/// Cek link masih hidup + ambil perannya (read-only — tak menghapus apa pun).
pub async fn invite_role(redis: &mut ConnectionManager, token: &str) -> Result<Option<String>> {
    let role: Option<String> = redis
        .get(invite_key(token))
        .await
        .map_err(|e| anyhow::anyhow!("Redis GET gagal: {e}"))?;
    Ok(role)
}

fn role_label(role: &str) -> &'static str {
    match role {
        "teacher" => "Guru",
        "dewan_guru" => "Dewan Guru",
        "santri" => "Santri",
        "parent" => "Orang Tua",
        _ => "Pengguna",
    }
}

/// Ajukan registrasi: validasi link, cek duplikat HP, rate-limit resend,
/// generate password+OTP, simpan pending 10 menit, kirim WA.
pub async fn initiate_register(
    pool: &Pool,
    redis: &mut ConnectionManager,
    http: &reqwest::Client,
    waha: &WahaConfig,
    token: &str,
    name: &str,
    phone: &str,
    profile: &StudentProfile,
) -> Result<()> {
    let Some(role) = invite_role(redis, token).await? else {
        bail_user!("Link registrasi tidak valid atau sudah kedaluwarsa.");
    };

    let name = name.trim();
    if name.chars().count() < 2 {
        bail_user!("Nama wajib diisi (minimal 2 karakter).");
    }
    let phone = normalize_local(phone)?;

    // Santri wajib melengkapi profil mahasiswa; peran lain tak diminta apa pun.
    let student = if crate::models::needs_student_profile(&role) {
        Some(validate_student_profile(profile)?)
    } else {
        None
    };

    // PENJAGA DUPLIKAT — memakai pencocokan yang TOLERAN BENTUK
    // (`find_account_by_phone`), bukan `find_by_phone` yang menyamakan teks
    // mentah.
    //
    // Bedanya menentukan siapa yang memiliki sebuah nomor. Baris lama bisa
    // tersimpan '0857…' sementara pendaftar mengetik nomor yang sama dan
    // dinormalkan jadi '62857…'. Perbandingan teks menyatakan keduanya
    // BERBEDA, `uq_users_phone` pun tak terusik karena stringnya memang tak
    // sama — jadi pendaftaran lolos dan nomor yang sama berakhir di DUA akun.
    // Sesudah itu login dan lupa-sandi harus menebak baris mana yang dimaksud,
    // dan orang yang bukan pemilik nomor memegang akun kedua atas nomor itu.
    //
    // Akun NONAKTIF ikut dihitung: nonaktif bukan berarti nomornya bebas
    // diambil orang lain — santri yang sedang cuti tetap pemiliknya.
    if repo::find_account_by_phone(pool, &phone).await?.is_some() {
        bail_user!("Nomor HP ini sudah terdaftar. Silakan masuk lewat halaman Login.");
    }

    let key = pending_key(&phone);
    if let Ok(Some(_)) = redis.get::<_, Option<String>>(&key).await {
        let ttl: i64 = redis.ttl(&key).await.unwrap_or(0);
        if ttl > 540 {
            bail_user!("OTP sudah dikirim. Tunggu {} detik lagi.", ttl - 540);
        }
    }

    let password = generate_random_password();
    let otp = format!("{:06}", rand::rng().random_range(100_000..=999_999));

    let (gender, campus, major, entry_year) = match student {
        Some((g, c, m, y)) => (g, c, m, Some(y)),
        None => (String::new(), String::new(), String::new(), None),
    };
    let pending = PendingRegistration {
        name: name.to_string(),
        phone: phone.clone(),
        role,
        password: password.clone(),
        otp: otp.clone(),
        gender,
        campus,
        major,
        entry_year,
        otp_attempts: 0,
    };
    let json = serde_json::to_string(&pending)?;
    let _: () = redis
        .set_ex(&key, json, 600u64)
        .await
        .map_err(|e| anyhow::anyhow!("Redis SET gagal: {e}"))?;

    send_wa_otp(http, waha, &phone, &otp, &password).await?;
    tracing::info!(phone = %phone, "OTP registrasi terkirim");
    Ok(())
}

/// Kirim ulang OTP — sama persis `initiate_register` (rate-limit di atas
/// otomatis berlaku), dipanggil dengan token+nama+HP yang sama.
pub async fn resend_otp(
    pool: &Pool,
    redis: &mut ConnectionManager,
    http: &reqwest::Client,
    waha: &WahaConfig,
    token: &str,
    name: &str,
    phone: &str,
    profile: &StudentProfile,
) -> Result<()> {
    initiate_register(pool, redis, http, waha, token, name, phone, profile).await
}

/// Cocokkan OTP → buat user (baru di sini Postgres tersentuh) → hapus kedua
/// key Redis (pending + link, sekali pakai) → JWT.
pub async fn verify_register(
    pool: &Pool,
    redis: &mut ConnectionManager,
    jwt: &crate::auth::JwtService,
    token: &str,
    phone: &str,
    otp_input: &str,
) -> Result<crate::service::auth::LoginOk> {
    let phone = normalize_local(phone)?;
    let key = pending_key(&phone);

    let json: Option<String> = redis
        .get(&key)
        .await
        .map_err(|e| anyhow::anyhow!("Redis GET gagal: {e}"))?;
    let Some(json) = json else {
        bail_user!("Sesi registrasi tidak ditemukan atau sudah kedaluwarsa.");
    };
    let mut pending: PendingRegistration =
        serde_json::from_str(&json).map_err(|e| anyhow::anyhow!("Data registrasi rusak: {e}"))?;

    if !constant_time_eq(&pending.otp, otp_input) {
        pending.otp_attempts = pending.otp_attempts.saturating_add(1);
        if pending.otp_attempts >= MAX_OTP_ATTEMPTS {
            // Buang pendaftarannya — penebak harus mulai dari awal, dan itu
            // kena batas laju kirim-ulang 60 detik yang sudah ada.
            let _: () = redis.del(&key).await.unwrap_or(());
            bail_user!("Terlalu banyak percobaan. Ulangi pendaftaran dari awal.");
        }
        // Simpan ulang TANPA memperpanjang umur: KEEPTTL menjaga sisa waktu
        // aslinya, jadi menebak berulang tak bisa memperpanjang jendela.
        if let Ok(j) = serde_json::to_string(&pending) {
            let _: Result<(), _> = redis
                .set_options(
                    &key,
                    j,
                    redis::SetOptions::default().with_expiration(redis::SetExpiry::KEEPTTL),
                )
                .await;
        }
        let sisa = MAX_OTP_ATTEMPTS - pending.otp_attempts;
        bail_user!("Kode OTP salah. Sisa {sisa} percobaan.");
    }

    // OTP sekali pakai: hapus SEBELUM insert (kegagalan insert di bawah tak boleh
    // membuat OTP bisa dipakai berulang).
    let _: () = redis.del(&key).await.unwrap_or(());

    // Konsumsi 1 kuota link (multi-pakai). DECR atomik → aman konkuren. Habis
    // (kuota 0) → hapus link. Legacy tanpa counter → sekali pakai (hapus).
    let has_counter: bool = redis.exists(uses_key(token)).await.unwrap_or(false);
    if has_counter {
        let remaining: i64 = redis.decr(uses_key(token), 1).await.unwrap_or(-1);
        if remaining < 0 {
            // Slot terakhir sudah diambil pendaftar lain (race) → kembalikan & tolak.
            let _: i64 = redis.incr(uses_key(token), 1).await.unwrap_or(0);
            bail_user!("Kuota link registrasi sudah habis. Minta link baru ke admin.");
        }
        if remaining == 0 {
            let _: () = redis.del(invite_key(token)).await.unwrap_or(());
            let _: () = redis.del(uses_key(token)).await.unwrap_or(());
        }
    } else {
        let _: () = redis.del(invite_key(token)).await.unwrap_or(());
    }

    let password = pending.password.clone();
    let hashed = tokio::task::spawn_blocking(move || bcrypt::hash(&password, 10)).await??;

    let opt = |s: &str| -> Option<String> {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    let user_id = repo::insert_registered_user(
        pool,
        &pending.name,
        &pending.phone,
        &pending.role,
        &hashed,
        opt(&pending.gender).as_deref(),
        opt(&pending.campus).as_deref(),
        opt(&pending.major).as_deref(),
        pending.entry_year,
    )
    .await?;

    let token = jwt.sign(user_id, &pending.name, &pending.phone, &pending.role)?;
    Ok(crate::service::auth::LoginOk {
        redirect: crate::models::role_home(&pending.role).to_string(),
        user: crate::models::SessionUser { id: user_id, name: pending.name, role: pending.role },
        token,
    })
}

/// Label peran (utk tampilan "Anda akan didaftarkan sebagai: …" di halaman
/// registrasi) — dipisah dari `invite_role` agar server fn bisa balas String
/// siap-tampil tanpa membuka logic peran mentah ke klien.
pub fn describe_role(role: &str) -> String {
    role_label(role).to_string()
}

// ── Internal ─────────────────────────────────────────────────────────────────

async fn send_wa_otp(
    http: &reqwest::Client,
    waha: &WahaConfig,
    phone: &str,
    otp: &str,
    password: &str,
) -> Result<()> {
    let text = format!(
        "Halo! Selamat datang di PPM AFM 🎉\n\n\
         Kode OTP kamu: *{}*\n\
         Password akun kamu: *{}*\n\n\
         ⚠️ Simpan password ini baik-baik.\n\
         OTP berlaku 10 menit. Jangan bagikan ke siapapun.",
        otp, password
    );
    send_wa_text(http, waha, phone, &text).await
}

/// Cek kesehatan WAHA + status sesi. Ok(status) mis. "WORKING"; Err(reason) bila
/// WAHA tak terjangkau / sesi bukan WORKING. Dipakai monitor Telegram (main.rs).
pub async fn waha_status(http: &reqwest::Client, waha: &WahaConfig) -> Result<String, String> {
    let url = format!("{}/api/sessions/{}", waha.base_url, waha.session);
    let mut req = http.get(&url);
    if !waha.api_key.is_empty() {
        req = req.header("X-Api-Key", &waha.api_key);
    }
    let res = req
        .send()
        .await
        .map_err(|e| format!("WAHA tak terjangkau ({url}): {e}"))?;
    if !res.status().is_success() {
        let code = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(format!("WAHA balas {code}: {body}"));
    }
    let v: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("WAHA respons tak valid: {e}"))?;
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("UNKNOWN");
    if status == "WORKING" {
        Ok(status.to_string())
    } else {
        Err(format!("Sesi WAHA '{}' berstatus {status} (bukan WORKING)", waha.session))
    }
}

/// Kirim pesan WhatsApp teks bebas via WAHA.
///
/// ── NOMORNYA DINORMALKAN DI SINI, SEKALI, UNTUK SEMUA PEMANGGIL ──────────────
/// Ini SATU-SATUNYA pintu keluar ke WhatsApp, dan ia pula yang menyusun
/// chat-id — jadi di sinilah aturan bentuk nomor semestinya berlaku, bukan di
/// tiap pemanggil.
///
/// Sebelum ini tiap pemanggil mengurusnya sendiri, dan hasilnya sudah menyimpang
/// persis seperti yang diperingatkan `models::phone`: `service::finance` dan
/// `service::permits` masing-masing punya salinan `wa_phone()` yang identik,
/// sementara `service::calendar` mengirim NILAI MENTAH DARI BASIS DATA. Untuk
/// baris yang tersimpan sebagai '0857…' — dan baris seperti itu ada, dari impor
/// daftar induk maupun isian lama — chat-id yang terbentuk adalah
/// `0857…@c.us`, yang bukan alamat siapa pun. WAHA menerimanya tanpa protes,
/// mengembalikan sukses, dan pesannya tak pernah sampai ke mana-mana.
///
/// Nomor yang TAK BISA ditafsirkan kini menjadi `Err` dengan sebab yang jelas,
/// bukan pesan yang dikirim ke alamat karangan: pemanggil sudah mencatat dan
/// menghitung kegagalan, jadi galat di sini terlihat — sementara "terkirim"
/// yang bohong tidak akan pernah terlihat.
///
/// Normalisasinya idempoten (lihat uji `idempoten` di `models::phone`), jadi
/// pemanggil yang sudah menormalkan lebih dulu tak berubah perilakunya.
pub async fn send_wa_text(
    http: &reqwest::Client,
    waha: &WahaConfig,
    phone: &str,
    text: &str,
) -> Result<()> {
    let chat_id = chat_id_untuk(phone)?;
    let body = serde_json::json!({
        "chatId": chat_id,
        "text": text,
        "session": waha.session,
    });
    let url = format!("{}/api/sendText", waha.base_url);
    let mut req = http.post(&url).json(&body);
    if !waha.api_key.is_empty() {
        req = req.header("X-Api-Key", &waha.api_key);
    }
    let res = req.send().await.map_err(|e| anyhow::anyhow!("Gagal menghubungi WAHA: {e}"))?;
    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        anyhow::bail!("WAHA error {status}: {body}");
    }
    Ok(())
}

/// Chat-id WAHA dari nomor dalam BENTUK APA PUN ("0857…", "+62 857…", "857…").
///
/// Dipisah dari [`send_wa_text`] supaya bisa diuji tanpa jaringan — inilah
/// bagian yang dulu diam-diam salah, dan bagian yang paling pantas dikunci tes.
fn chat_id_untuk(phone: &str) -> Result<String> {
    match crate::models::normalisasi_hp(phone) {
        Some(hp) => Ok(crate::models::chat_id_wa(&hp)),
        None => anyhow::bail!(
            "Nomor WhatsApp tak bisa ditafsirkan: {phone:?}.              Betulkan nomornya di /manajemen-user."
        ),
    }
}

/// "08xxx"/"+62xxx"/"62xxx" → "62xxx" (dipakai sbg identitas HP tersimpan DAN
/// basis chat-ID WAHA, konsisten — beda dari e-ticketing yg normalisasi
/// terpisah utk simpan vs kirim WA).
fn normalize_local(phone: &str) -> Result<String> {
    match crate::models::normalisasi_hp(phone) {
        Some(hp) => Ok(hp),
        None => bail_user!("{}", crate::models::pesan_hp_tidak_sah()),
    }
}

pub(crate) fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Password acak mudah dibaca dari WA — persis pola e-ticketing: 9 karakter,
/// Upper·Lower×3·Digit·Special·Lower×3, tanpa karakter ambigu (I/O/i/l/o/0/1).
pub fn generate_random_password() -> String {
    const UPPER: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghjkmnpqrstuvwxyz";
    const DIGITS: &[u8] = b"23456789";
    const SPECIAL: &[u8] = b"@#$%&";

    let mut rng = rand::rng();
    let mut pass = Vec::with_capacity(9);
    pass.push(UPPER[rng.random_range(0..UPPER.len())] as char);
    for _ in 0..3 {
        pass.push(LOWER[rng.random_range(0..LOWER.len())] as char);
    }
    pass.push(DIGITS[rng.random_range(0..DIGITS.len())] as char);
    pass.push(SPECIAL[rng.random_range(0..SPECIAL.len())] as char);
    for _ in 0..3 {
        pass.push(LOWER[rng.random_range(0..LOWER.len())] as char);
    }
    pass.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Yang dikunci di sini adalah PENYEBAB kegagalan diam-diam: chat-id yang
    /// terbentuk dari nilai basis data apa adanya. WAHA menerima
    /// `0857…@c.us` tanpa protes dan menjawab sukses, jadi tak ada satu pun
    /// lapisan sesudahnya yang bisa menangkapnya.
    #[test]
    fn chat_id_selalu_bentuk_62_apa_pun_masukannya() {
        let harapan = "6281234567890@c.us";
        for masukan in [
            "081234567890",
            "6281234567890",
            "+62 812-3456-7890",
            "+62 0812 3456 7890",
            "81234567890",
            "  0812 3456 7890  ",
        ] {
            assert_eq!(chat_id_untuk(masukan).unwrap(), harapan, "masukan {masukan}");
        }
    }

    #[test]
    fn nomor_tak_tertafsirkan_jadi_galat_bukan_alamat_karangan() {
        // Nomor rumah, potongan angka, kolom kosong — semuanya dulu berakhir
        // sebagai chat-id yang tampak sah lalu hilang tanpa jejak.
        for masukan in ["0217654321", "0812", "", "-"] {
            assert!(chat_id_untuk(masukan).is_err(), "masukan {masukan:?} seharusnya ditolak");
        }
    }
}
