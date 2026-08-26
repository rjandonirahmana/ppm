//! service/auth.rs — Login (bcrypt verify → JWT), lupa-sandi, bootstrap admin.
//!
//! ── SANDI HASIL "LUPA SANDI" MENUNGGU DIPAKAI ────────────────────────────────
//! `forgot_password` TIDAK menyentuh Postgres. Sandi baru dikirim lewat WA dan
//! hash-nya dititipkan di Redis selama tiga jam; `users.password_hash` baru
//! berubah pada login pertama yang benar-benar memakainya (`pakai_sandi_menunggu`).
//! Selama jendela itu sandi LAMA tetap berlaku, dan bila tak ada yang memakai
//! sandi barunya, tak ada apa pun yang berubah.
//!
//! Alasannya ada di badan `pakai_sandi_menunggu`: menekan "lupa sandi" hanya
//! butuh mengetik nomor HP orang lain, jadi reset yang langsung menimpa DB
//! adalah tombol untuk mengunci siapa pun keluar dari akunnya.

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

/// Normalisasi input jadi bentuk HP tersimpan. Dipakai login (mencocokkan
/// `phone_number`) & lupa-sandi.
///
/// Meneruskan ke [`crate::models::normalisasi_hp`] — SATU aturan untuk seluruh
/// aplikasi. Empat salinan yang dulu berdiri sendiri membuat nomor yang sama
/// tersimpan berbeda tergantung pintu masuknya, dan pencarian yang
/// membandingkan teks tak menemukannya.
///
/// Masukan yang tak bisa ditafsirkan dikembalikan sebagai digitnya saja, BUKAN
/// ditolak: fungsi ini juga dipakai MENCARI, dan pencarian yang gagal harus
/// berakhir "tak ada yang cocok", bukan galat.
pub fn normalize_phone(s: &str) -> String {
    crate::models::normalisasi_hp(s)
        .unwrap_or_else(|| s.chars().filter(|c| c.is_ascii_digit()).collect())
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

/// Umur sandi hasil "lupa sandi" selama ia belum dipakai. Tiga jam: cukup untuk
/// membuka WhatsApp yang tertunda, cukup pendek untuk tidak menggantung.
const SANDI_MENUNGGU_TTL: u64 = 3 * 60 * 60;

/// Kunci Redis berisi HASH bcrypt sandi baru yang belum dipakai masuk.
///
/// Yang disimpan hash-nya, bukan sandinya. Redis di sini bukan brankas: ia
/// dipakai juga untuk cache, dan siapa pun yang bisa membacanya jangan sampai
/// sekaligus memperoleh sandi yang bisa langsung diketik di layar login.
fn sandi_menunggu_key(user_id: i64) -> String {
    format!("pw-baru:ppm:{user_id}")
}

/// Lepas kunci batas laju lupa-sandi supaya percobaan berikutnya tak ikut
/// tertahan 10 menit. Dipanggil hanya di jalur GAGAL — lihat `forgot_password`.
async fn lepas_batas_laju(redis: &mut redis::aio::ConnectionManager, key: &str) {
    use redis::AsyncCommands;
    if let Err(e) = redis.del::<_, ()>(key).await {
        tracing::warn!("gagal melepas batas laju {key}: {e}");
    }
}

/// Buang sandi-menunggu milik seseorang. Best-effort: Redis bermasalah tak
/// boleh menggagalkan login/ganti sandi yang sudah berhasil.
async fn buang_sandi_menunggu(redis: &mut redis::aio::ConnectionManager, user_id: i64) {
    use redis::AsyncCommands;
    if let Err(e) = redis.del::<_, ()>(sandi_menunggu_key(user_id)).await {
        tracing::warn!("gagal membuang sandi-menunggu {user_id}: {e}");
    }
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
    // Sandi awal/reset dikirim lewat WhatsApp, dan menyalin dari bubble WA
    // sangat sering ikut membawa spasi/newline — akibatnya tempel-then-login
    // gagal sementara ketik manual berhasil ("kadang bisa kadang tidak").
    let password = password.trim();
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
        // Sandi di DB tak cocok — tapi mungkin yang diketik ini SANDI BARU dari
        // "lupa sandi" yang belum pernah dipakai. Di titik inilah, dan hanya di
        // sini, sandi itu benar-benar menggantikan yang lama.
        if !pakai_sandi_menunggu(pool, redis, user.id, password).await? {
            note_login_failure(redis, &attempt_key).await;
            bail_user!("Nomor HP atau kata sandi salah.");
        }
        tracing::info!(user_id = user.id, "sandi baru dari lupa-sandi dipakai — DB diperbarui");
    } else {
        // Masuk dengan sandi LAMA berarti pemiliknya tak pernah kehilangan
        // akses. Sandi yang sempat dikirim ke WA dibatalkan sekarang juga:
        // yang dipakai lebih dulu menang, dan tak ada sandi kedua yang
        // menggantung selama tiga jam untuk akun yang jelas masih dipegang.
        buang_sandi_menunggu(redis, user.id).await;
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

/// Cocokkan `password` dengan sandi-menunggu milik `user_id`. Bila cocok:
/// TULIS ke `users.password_hash` dan buang key-nya, lalu `true`.
///
/// ── KENAPA SANDI RESET TIDAK LANGSUNG DITULIS KE DB ──────────────────────────
/// Menekan "lupa sandi" hanya butuh mengetik nomor HP orang lain. Bila reset
/// langsung menimpa DB, siapa pun bisa mengunci siapa pun keluar dari akunnya:
/// korban tak melakukan apa-apa, sandinya sudah tidak berlaku, dan sandi
/// penggantinya ada di WhatsApp yang mungkin tak pernah ia buka — atau, seperti
/// yang berulang kali terjadi di pondok, tak pernah sampai.
///
/// Karena itu sandi baru MENUNGGU: selama tiga jam ia hidup di Redis dan DUA
/// sandi sama-sama berlaku — yang lama dan yang baru. Yang menentukan bukan
/// permintaan resetnya, melainkan sandi mana yang benar-benar dipakai masuk.
/// Tak dipakai sama sekali → key-nya kedaluwarsa sendiri dan sandi di DB tak
/// pernah tersentuh.
///
/// Biaya yang diterima: satu bcrypt tambahan pada login yang gagal, dan hanya
/// bila memang ada sandi-menunggu (GET dulu, verify belakangan).
async fn pakai_sandi_menunggu(
    pool: &Pool,
    redis: &mut redis::aio::ConnectionManager,
    user_id: i64,
    password: &str,
) -> Result<bool> {
    use redis::AsyncCommands;
    let key = sandi_menunggu_key(user_id);
    // Redis mati → tak ada sandi-menunggu yang bisa dibaca; login jalan seperti
    // biasa dengan sandi DB. Jangan menggagalkan seluruh login karenanya.
    let hash: Option<String> = match redis.get(&key).await {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!("gagal membaca sandi-menunggu {user_id}: {e}");
            return Ok(false);
        }
    };
    let Some(hash) = hash else { return Ok(false) };

    let pw = password.to_string();
    let hash_verify = hash.clone();
    if !tokio::task::spawn_blocking(move || bcrypt::verify(&pw, &hash_verify)).await?? {
        return Ok(false);
    }

    // DITULIS DULU, baru key-nya dibuang. Urutan sebaliknya membuat sandi baru
    // lenyap bila penulisan DB gagal — dan pemiliknya tinggal dengan sandi lama
    // yang barusan ia yakini sudah diganti.
    repo::set_password_hash(pool, user_id, &hash).await?;
    buang_sandi_menunggu(redis, user_id).await;
    Ok(true)
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

/// Forgot-password via WA: cari akun dari nomor HP → buat sandi baru → kirim
/// lewat WhatsApp → simpan HASH-nya di Redis selama tiga jam. Return kalimat
/// yang bisa langsung ditampilkan ke layar.
///
/// **Basis data TIDAK disentuh di sini.** Sandi lama tetap berlaku sampai
/// seseorang benar-benar masuk memakai sandi barunya — lihat
/// [`pakai_sandi_menunggu`] untuk alasan lengkapnya.
///
/// ── KENAPA SEKARANG BERTERUS TERANG SOAL NOMOR YANG TAK TERDAFTAR ────────────
/// Versi sebelumnya SELALU menjawab "berhasil" apa pun kenyataannya —
/// anti-enumerasi, supaya nomor mana yang punya akun tak bisa dipetakan orang
/// luar. Yang tampak di lapangan: nomor salah ketik, akun nonaktif, dan WAHA
/// mati menghasilkan layar yang sama persis dengan pengiriman yang berhasil,
/// dan orang menunggu WA yang tak akan pernah datang. Untuk pondok dengan
/// beberapa ratus penghuni, kerugian itu nyata setiap hari sementara ancaman
/// enumerasinya tidak.
///
/// Jadi pilihannya dibalik dengan sadar: **jawaban jujur**, dan yang menahan
/// penyalahgunaan adalah batas laju per nomor di bawah — bukan kekaburan
/// pesan. Konsekuensinya diterima: seseorang kini bisa menguji apakah sebuah
/// nomor punya akun di sini.
pub async fn forgot_password(
    pool: &Pool,
    redis: &mut redis::aio::ConnectionManager,
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    phone: &str,
) -> Result<String> {
    let masukan = phone.trim().to_string();
    let phone = normalize_phone(&masukan);
    // Bukan nomor Indonesia yang bisa ditafsirkan → katakan, jangan diamkan.
    if crate::models::normalisasi_hp(&masukan).is_none() {
        bail_user!("{}", crate::models::pesan_hp_tidak_sah());
    }

    // PENCARIAN DULU, batas laju belakangan. Urutan ini yang membuat salah
    // ketik bisa dibetulkan seketika: nomor yang tak dikenal dijawab langsung
    // dan TIDAK menghabiskan jatah 10 menit, sehingga percobaan berikutnya
    // dengan nomor yang benar tak ikut tertahan.
    //
    // Pencocokannya HARUS sama dengan pencarian login — lihat
    // `repo::find_account_by_phone`. Versi lama memakai `find_by_phone` yang
    // menyamakan teks mentah, jadi nomor yang tersimpan sebagai '0858…' tak
    // pernah ditemukan dan resetnya berakhir di sana, diam-diam.
    let Some((user_id, aktif)) = repo::find_account_by_phone(pool, &phone).await? else {
        tracing::info!("forgot_password: tak ada akun dengan nomor {phone}");
        bail_user!(
            "Nomor {} belum terdaftar di AFM SMART. Periksa lagi nomornya, atau hubungi \
             pengurus bila nomor Anda sudah berganti.",
            super::ganti_nomor::samarkan(&phone)
        );
    };
    if !aktif {
        tracing::warn!(user_id, "forgot_password: akun {phone} NONAKTIF");
        bail_user!(
            "Akun dengan nomor ini sedang NONAKTIF, jadi sandinya tak bisa direset. \
             Hubungi pengurus untuk mengaktifkannya kembali."
        );
    }

    // BATAS LAJU per nomor. Tanpa ini siapa saja bisa me-reset sandi nomor
    // korban berulang kali: korban dibanjiri WA DAN sandinya berganti terus.
    // Inilah — bukan kekaburan pesan — yang menahan penyalahgunaan sekarang.
    //
    // 10 MENIT ITU LAMA bila pengiriman baru saja GAGAL: orangnya menekan
    // "kirim ulang" dan tak terjadi apa-apa. Karena itu kuncinya DILEPAS lagi
    // di setiap jalur kegagalan di bawah — yang ditahan adalah pengiriman yang
    // berhasil, bukan yang gagal.
    let batas_key = format!("fp:{phone}");
    {
        use redis::AsyncCommands;
        let fresh: Option<bool> = redis
            .set_options(
                &batas_key,
                1i32,
                redis::SetOptions::default()
                    .conditional_set(redis::ExistenceCheck::NX)
                    .with_expiration(redis::SetExpiry::EX(600)),
            )
            .await
            .unwrap_or(None);
        if fresh.is_none() {
            tracing::info!("forgot_password: {phone} ditahan batas laju");
            bail_user!(
                "Sandi baru sudah dikirim ke WhatsApp Anda kurang dari 10 menit yang lalu. \
                 Periksa pesan WhatsApp dulu; bila memang belum ada, coba lagi setelah \
                 10 menit."
            );
        }
    }

    let new_pw = super::registration::generate_random_password();
    let pw = new_pw.clone();
    let hash = tokio::task::spawn_blocking(move || bcrypt::hash(&pw, 10)).await??;

    // KIRIM DULU, baru simpan. Urutan sebaliknya menyisakan sandi-menunggu
    // untuk pesan yang tak pernah terkirim — tak merusak apa pun (sandi lama
    // tetap jalan), tapi menaruh sandi yang tak diketahui siapa pun di Redis
    // selama tiga jam tak ada gunanya.
    let msg = format!(
        "🔑 *Reset Password AFM SMART*\nPassword baru Anda: *{new_pw}*\n\n\
         Berlaku 3 jam. Password LAMA masih bisa dipakai — yang baru menggantikannya \
         hanya setelah Anda berhasil masuk dengan password ini.\n\n\
         Masuk dengan nomor HP + password di atas, lalu ganti password di menu Profil."
    );
    if let Err(e) = super::registration::send_wa_text(http, waha, &phone, &msg).await {
        tracing::error!("forgot_password: WA gagal ke {phone} — sandi TIDAK diubah: {e:#}");
        // DIALARMKAN, bukan sekadar dicatat: bila WAHA mati, SELURUH pemulihan
        // akun berhenti bekerja, dan pengelola harus tahu lebih dulu daripada
        // orang yang sedang terkunci di luar akunnya.
        super::telegram::report_background_error(
            "Lupa sandi: WA gagal",
            format!("Tujuan {phone}: {e:#}"),
        );
        lepas_batas_laju(redis, &batas_key).await;
        bail_user!(
            "Nomornya terdaftar, tapi pesan WhatsApp-nya gagal terkirim. Coba lagi \
             beberapa saat; bila tetap gagal, hubungi pengurus."
        );
    }

    // Hanya di Redis, dan hanya hash-nya. Postgres tak disentuh sama sekali.
    {
        use redis::AsyncCommands;
        if let Err(e) = redis
            .set_ex::<_, _, ()>(sandi_menunggu_key(user_id), &hash, SANDI_MENUNGGU_TTL)
            .await
        {
            // Sandinya sudah telanjur terkirim lewat WA tapi tak bisa disimpan →
            // ia tak akan bisa dipakai. Dikeraskan jadi galat (bukan Ok diam)
            // supaya masuk log & alarm: pengguna yang mengira sudah punya sandi
            // baru padahal tidak adalah persis keluhan yang sedang diperbaiki.
            lepas_batas_laju(redis, &batas_key).await;
            return Err(anyhow::anyhow!("gagal menyimpan sandi-menunggu: {e}"));
        }
    }

    tracing::info!(user_id, "forgot_password: sandi baru dikirim ke {phone}");
    Ok(format!(
        "Password baru dikirim ke WhatsApp {}. Berlaku 3 jam, dan password lama masih \
         bisa dipakai sampai Anda masuk dengan yang baru.",
        super::ganti_nomor::samarkan(&phone)
    ))
}

/// Ganti kata sandi user yang sedang login: cocokkan sandi LAMA (bcrypt verify),
/// bila cocok simpan sandi BARU (bcrypt hash). bcrypt di `spawn_blocking`.
///
/// Sandi-menunggu (bila ada) ikut dibuang: orang ini jelas memegang akunnya dan
/// baru saja memilih sandinya sendiri — sandi dari WA yang belum sempat dipakai
/// tak boleh tetap sah sesudah itu.
pub async fn change_password(
    pool: &Pool,
    redis: &mut redis::aio::ConnectionManager,
    user_id: i64,
    old: &str,
    new: &str,
) -> Result<()> {
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
    buang_sandi_menunggu(redis, user_id).await;
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
