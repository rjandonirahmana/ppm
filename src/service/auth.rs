//! service/auth.rs — Login (bcrypt verify → JWT) + bootstrap admin.

use anyhow::Result;
use deadpool_postgres::Pool;

use crate::auth::JwtService;
use crate::models::SessionUser;
use crate::repository as repo;

/// Hasil login sukses.
pub struct LoginOk {
    pub user: SessionUser,
    pub token: String,
    /// Path redirect sesuai peran.
    pub redirect: String,
}

/// Normalisasi input jadi bentuk HP tersimpan (08.. → 62..). Non-digit dibuang.
/// Dipakai login (cocokkan phone_number) & forgot-password.
pub fn normalize_phone(s: &str) -> String {
    let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
    match d.strip_prefix('0') {
        Some(rest) => format!("62{rest}"),
        None => d,
    }
}

/// Hash bcrypt boneka (cost 10, sama dgn hash asli) untuk menyamakan waktu
/// respons saat user tidak ditemukan — lihat `login`. Bukan sandi siapa pun.
const DUMMY_HASH: &str = "$2b$10$N9qo8uLOickgx2ZMRZoMyeIjZAgcfl7p92ldGxad68LJZdL17lhWy";

/// Percobaan login gagal yang ditoleransi sebelum akun dikunci sementara.
const LOGIN_MAX_ATTEMPTS: u32 = 10;
/// Lama jendela hitung & lama kunci setelah batas terlampaui (detik).
const LOGIN_LOCK_SECS: u64 = 900; // 15 menit

fn login_attempt_key(identifier: &str) -> String {
    format!("login_fail:{identifier}")
}

/// Verifikasi kredensial → JWT (pola sama e-ticketing AuthService::login).
/// Login UTAMANYA pakai NOMOR HP; username/email/NIS tetap didukung (admin seed).
/// bcrypt CPU-bound → `spawn_blocking` agar tidak menyumbat worker async.
///
/// BATAS LAJU per identitas (Redis, pola sama `forgot_password`). Sebelumnya
/// login sama sekali tak dibatasi, sehingga siapa pun bisa menebak sandi sebuah
/// nomor tanpa henti — sandi awal yang dibagikan sistem hanya 8 karakter acak,
/// jadi penebakan tanpa batas bukan ancaman teoretis. Efek kedua yang sama
/// pentingnya: tiap percobaan memicu satu bcrypt cost-10 (~80 ms CPU) di
/// `spawn_blocking`; tanpa batas, banjir percobaan bersamaan menguras kolam
/// thread blocking dan membuat SELURUH aplikasi tak responsif di VPS 2 CPU.
///
/// Dihitung per IDENTITAS, bukan per IP: aplikasi berada di belakang proxy dan
/// santri berbagi WiFi pondok — membatasi per IP akan mengunci satu asrama
/// sekaligus. Konsekuensi yang diterima: seseorang bisa mengunci akun orang lain
/// selama 15 menit. Itu sebabnya jendelanya pendek dan pulih sendiri, dan
/// `forgot_password` (jalur pemulihan) memakai batas laju terpisah.
pub async fn login(
    pool: &Pool,
    redis: &mut redis::aio::ConnectionManager,
    jwt: &JwtService,
    login: &str,
    password: &str,
) -> Result<LoginOk> {
    let login = login.trim();
    let phone = normalize_phone(login);

    // Kunci hitungan memakai bentuk ternormalisasi bila input berupa nomor HP,
    // supaya "0812…" dan "62812…" tidak dihitung sebagai dua sasaran berbeda.
    let attempt_key = login_attempt_key(if phone.len() >= 8 { &phone } else { login });

    // Dicek SEBELUM query DB & bcrypt — supaya penolakan tidak berbiaya.
    // Redis mati → `unwrap_or(0)` = lolos (fail-open): pemadaman Redis tak boleh
    // mengunci seluruh pengguna dari aplikasinya sendiri.
    {
        use redis::AsyncCommands;
        let fails: u32 = redis.get(&attempt_key).await.unwrap_or(0);
        if fails >= LOGIN_MAX_ATTEMPTS {
            tracing::warn!("login ditahan batas laju untuk {attempt_key}");
            bail_user!(
                "Terlalu banyak percobaan masuk yang gagal. Coba lagi sekitar 15 menit, \
                 atau gunakan \"Lupa kata sandi\"."
            );
        }
    }

    let Some(user) = repo::find_user_for_login(pool, login, &phone).await? else {
        // Tetap jalankan bcrypt terhadap hash boneka. Tanpa ini, login untuk
        // nomor yang TIDAK terdaftar balas seketika sementara yang terdaftar
        // butuh ~80 ms — selisih itu cukup untuk memetakan nomor mana saja yang
        // punya akun (user enumeration) tanpa perlu menebak sandinya.
        let pw = password.to_string();
        let _ = tokio::task::spawn_blocking(move || bcrypt::verify(&pw, DUMMY_HASH)).await;
        // Dihitung juga saat nomor tak terdaftar — kalau tidak, penyerang bisa
        // menyaring nomor yang ADA hanya dari mana yang kena kunci lebih dulu.
        note_login_failure(redis, &attempt_key).await;
        bail_user!("Nomor HP atau kata sandi salah.");
    };

    let hash = user.password_hash.clone();
    let pw = password.to_string();
    let verify_start = std::time::Instant::now();
    let ok = tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &hash)).await??;
    // DEBUG, bukan INFO: tiap login menuliskan baris ini dan nilainya hanya
    // berguna saat menyetel biaya bcrypt, bukan di log produksi sehari-hari.
    tracing::debug!(
        verify_ms = verify_start.elapsed().as_millis(),
        "bcrypt verify done"
    );
    if !ok {
        note_login_failure(redis, &attempt_key).await;
        bail_user!("Nomor HP atau kata sandi salah.");
    }

    // Berhasil → hitungan dinolkan, supaya kegagalan yang tersebar sepanjang
    // hari (salah ketik biasa) tak pernah menumpuk sampai mengunci pengguna sah.
    {
        use redis::AsyncCommands;
        let _: Result<(), _> = redis.del::<_, ()>(&attempt_key).await;
    }

    let phone = user.phone_number.clone().unwrap_or_default();
    let token = jwt.sign(user.id, &user.full_name, &phone, &user.role)?;
    Ok(LoginOk {
        redirect: crate::models::role_home(&user.role).to_string(),
        user: SessionUser {
            id: user.id,
            name: user.full_name,
            role: user.role,
        },
        token,
    })
}

/// Catat satu kegagalan login. INCR lalu set TTL saat hitungan pertama, jadi
/// jendelanya bergulir dari kegagalan pertama dan hitungan hilang sendiri —
/// tak ada yang terkunci selamanya walau tak pernah berhasil masuk.
/// Best-effort: Redis bermasalah tak boleh menggagalkan proses login.
async fn note_login_failure(redis: &mut redis::aio::ConnectionManager, key: &str) {
    use redis::AsyncCommands;
    match redis.incr::<_, _, u32>(key, 1u32).await {
        Ok(1) => {
            let _: Result<(), _> = redis.expire::<_, ()>(key, LOGIN_LOCK_SECS as i64).await;
        }
        Ok(_) => {}
        Err(e) => tracing::warn!("gagal mencatat kegagalan login: {e}"),
    }
}

/// Forgot-password via WA: cari user dari nomor HP → buat password baru → kirim
/// lewat WhatsApp. Best-effort & anti-enumerasi: SELALU balas Ok (tak bocorkan
/// apakah nomor terdaftar). bcrypt di `spawn_blocking` (CPU-bound).
pub async fn forgot_password(
    pool: &Pool,
    redis: &mut redis::aio::ConnectionManager,
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    phone: &str,
) -> Result<()> {
    let phone = normalize_phone(phone);
    if phone.len() < 8 {
        return Ok(()); // input tak masuk akal → diam
    }

    // BATAS LAJU, sebelum menyentuh apa pun. Tanpa ini siapa saja bisa me-reset
    // sandi nomor korban berulang kali: korban dibanjiri WA DAN sandinya
    // berganti terus. Balasan tetap Ok agar tak membocorkan nomor terdaftar.
    {
        use redis::AsyncCommands;
        let key = format!("fp:{phone}");
        let fresh: Option<bool> = redis
            .set_options(
                &key,
                1i32,
                redis::SetOptions::default()
                    .conditional_set(redis::ExistenceCheck::NX)
                    .with_expiration(redis::SetExpiry::EX(600)),
            )
            .await
            .unwrap_or(None);
        if fresh.is_none() {
            tracing::info!("forgot_password: ditahan batas laju untuk {phone}");
            return Ok(());
        }
    }

    let Some(user_id) = repo::find_by_phone(pool, &phone).await? else {
        return Ok(()); // tak terdaftar → diam (anti-enumerasi)
    };

    let new_pw = super::registration::generate_random_password();
    let pw = new_pw.clone();
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 10)).await??;

    // KIRIM DULU, baru ganti sandi. Urutan sebaliknya (dulu begitu) membuat
    // pengguna TERKUNCI saat WAHA mati: sandi lamanya sudah tak berlaku,
    // sandi barunya tak pernah sampai. Lebih baik reset gagal diam-diam dan
    // bisa dicoba lagi daripada seseorang kehilangan akses.
    let msg = format!(
        "🔑 *Reset Password PPM AFM*\nPassword baru Anda: *{new_pw}*\n\nMasuk dengan nomor HP + password ini, lalu segera ganti password di menu Profil."
    );
    if let Err(e) = super::registration::send_wa_text(http, waha, &phone, &msg).await {
        tracing::warn!("forgot_password: WA gagal ke {phone} — sandi TIDAK diubah: {e}");
        return Ok(());
    }
    repo::set_password_hash(pool, user_id, &hash).await?;
    Ok(())
}

/// Ganti kata sandi user yang sedang login: cocokkan sandi LAMA (bcrypt verify),
/// bila cocok simpan sandi BARU (bcrypt hash). bcrypt di `spawn_blocking`.
pub async fn change_password(pool: &Pool, user_id: i64, old: &str, new: &str) -> Result<()> {
    if new.chars().count() < 6 {
        bail_user!("Kata sandi baru minimal 6 karakter.");
    }
    if new == old {
        bail_user!("Kata sandi baru harus berbeda dari yang lama.");
    }
    let Some(hash) = repo::get_password_hash(pool, user_id).await? else {
        bail_user!("Akun tidak ditemukan.");
    };
    let old_s = old.to_string();
    let ok = tokio::task::spawn_blocking(move || bcrypt::verify(&old_s, &hash)).await??;
    if !ok {
        bail_user!("Kata sandi lama salah.");
    }
    let new_s = new.to_string();
    let new_hash = tokio::task::spawn_blocking(move || bcrypt::hash(&new_s, 10)).await??;
    repo::set_password_hash(pool, user_id, &new_hash).await?;
    Ok(())
}

/// Bootstrap: bila tabel users KOSONG, buat admin awal
/// (username `admin`, password dari env ADMIN_PASSWORD; default "admin123"
/// HANYA di luar produksi — lihat badan fungsi).
/// Tidak menyentuh apa pun bila sudah ada data (aman utk DB yang sedang diisi).
pub async fn ensure_seed_admin(pool: &Pool) -> Result<()> {
    if repo::count_users(pool).await? > 0 {
        return Ok(());
    }
    // "admin123" hanya boleh untuk dev. Di produksi (LEPTOS_ENV=PROD) admin
    // pertama WAJIB punya sandi dari env — kalau tidak, instalasi baru berdiri
    // dengan sandi admin yang tertulis di kode sumber.
    let pw = match std::env::var("ADMIN_PASSWORD") {
        Ok(p) if !p.trim().is_empty() => p,
        _ => {
            if std::env::var("LEPTOS_ENV").as_deref() == Ok("PROD") {
                bail_user!(
                    "ADMIN_PASSWORD wajib diset saat pertama kali menjalankan di \
                     produksi (tabel users masih kosong)."
                );
            }
            "admin123".into()
        }
    };
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 10)).await??;
    repo::insert_admin(pool, &hash).await?;
    tracing::info!("Seed admin dibuat (username: admin — ganti password segera)");
    Ok(())
}
