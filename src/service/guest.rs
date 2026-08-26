//! service/guest.rs — Buku tamu berbasis KODE di Redis (migrasi 35).
//!
//! Alur:
//!   1. Tamu isi /tamu → `register_guest` → kode 6-digit disimpan
//!      `tamu:code:{kode}` = JSON data tamu (TTL 12 jam ≈ "hari itu"), lalu
//!      kodenya DIKIRIM KE WHATSAPP nomor yang ia tulis. Layar hanya menerima
//!      `tamu:tiket:{tiket}` — pengenal acak untuk menunggu konfirmasi.
//!   2. Tamu ketik kode di mesin IoT → mesin kirim {api_key, code, foto} →
//!      `consume_guest` cari kode → hapus → (handler) simpan kunjungan+wajah →
//!      `mark_done` set `tamu:done:{kode}` (TTL 10 mnt) untuk polling HP.
//!   3. Halaman /tamu polling `check_status` dengan TIKET → tampil ✅ + wajah.
//!
//! ── KENAPA KODENYA TAK BOLEH TAMPIL DI LAYAR ─────────────────────────────────
//! Rancangan pertama memajang kodenya di halaman tamu. Akibatnya isian "Nomor
//! HP" tak membuktikan apa pun: siapa pun bisa menulis nomor karangan — atau
//! nomor orang lain — lalu tetap masuk. Untuk sebuah buku tamu, nomor yang tak
//! bisa dihubungi sama nilainya dengan kolom kosong; yang tersisa hanyalah rasa
//! aman bahwa "datanya ada".
//!
//! Dengan kode hanya lewat WhatsApp, mengetiknya di gerbang OTOMATIS menjadi
//! bukti bahwa tamu memegang nomor itu. Tak ada langkah verifikasi tambahan
//! yang perlu dijalani siapa pun — pembuktiannya menyatu dengan langkah yang
//! toh harus ia lakukan.

use anyhow::Result;
use rand::RngExt;
use redis::{aio::ConnectionManager, AsyncCommands};
use serde::{Deserialize, Serialize};

use crate::models::{GuestCheckin, GuestTicket};

/// TTL kode tamu: 12 jam (cukup untuk satu hari kunjungan).
const CODE_TTL: u64 = 12 * 3600;
/// TTL penanda "sukses" agar HP tamu sempat polling & lihat ✅.
const DONE_TTL: u64 = 600;

/// Jeda minimum antar pengiriman WA ke SATU nomor tamu (detik).
///
/// /tamu adalah endpoint PUBLIK tanpa login — siapa pun di internet bisa
/// memanggilnya dengan nomor siapa pun. Tanpa batas ini, ia adalah alat kirim
/// WhatsApp gratis ke nomor mana saja, sebanyak yang diinginkan penyerang, atas
/// nama pondok — dan yang menanggung akibatnya nomor WAHA pondok sendiri, yang
/// bisa diblokir WhatsApp karena dianggap spam.
///
/// KODENYA TETAP DIBUAT saat batas ini kena; yang ditahan hanya pesannya.
/// Tamu sungguhan yang mengisi ulang formulir tetap bisa check-in — kodenya
/// ada di layarnya.
const WA_JEDA_DETIK: u64 = 600;

#[derive(Serialize, Deserialize)]
pub struct PendingGuest {
    pub name: String,
    pub phone: String,
    pub purpose: String,
}

fn code_key(code: &str) -> String {
    format!("tamu:code:{code}")
}
fn done_key(code: &str) -> String {
    format!("tamu:done:{code}")
}

/// Tiket polling → kode check-in. Dipegang browser tamu; tak berguna di gerbang.
fn tiket_key(ticket: &str) -> String {
    format!("tamu:tiket:{ticket}")
}

/// Nomor tamu → kode yang masih berlaku untuknya. SATU kode aktif per nomor.
///
/// Tanpa ini, tamu yang mengisi formulir dua kali (salah ketik nama, halaman
/// ter-refresh, jaringan putus) mendapat kode kedua sementara yang pertama
/// masih hidup 12 jam — dua kode sah untuk satu orang, dan yang di WhatsApp-nya
/// belum tentu yang ia coba ketik di gerbang.
fn hp_key(phone: &str) -> String {
    format!("tamu:hp:{phone}")
}

fn gen_tiket() -> String {
    let mut b = [0u8; 16];
    rand::rng().fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn gen_code() -> String {
    format!("{:06}", rand::rng().random_range(0..=999_999))
}

/// Daftarkan tamu → kode unik 6-digit, sekaligus kirim kodenya lewat WhatsApp.
/// Retry bila tabrakan kode.
///
/// ── WHATSAPP DI SINI BEST-EFFORT, BEDA DARI ALUR LAIN ────────────────────────
/// Pada registrasi akun dan lupa-sandi, WhatsApp adalah SATU-SATUNYA jalan
/// kodenya sampai — gagal kirim berarti alurnya berhenti. Di sini tidak:
/// kodenya sudah tampil di layar tamu sebelum WhatsApp disentuh sama sekali.
/// Karena itu kegagalan WA TIDAK menggagalkan pendaftaran — ia hanya dilaporkan
/// (log + alarm), dan layar diberi tahu lewat `wa_terkirim` supaya tak
/// menjanjikan pesan yang tak pernah dikirim.
pub async fn register_guest(
    redis: &mut ConnectionManager,
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    name: &str,
    phone: &str,
    purpose: &str,
) -> Result<GuestTicket> {
    let name = name.trim();
    if name.is_empty() {
        bail_user!("Nama wajib diisi.");
    }
    // Nomor tamu DINORMALKAN seperti nomor lain di aplikasi ini. Sebelumnya ia
    // disimpan mentah — hanya `trim()` — sehingga tamu yang menulis "0812…"
    // tersimpan apa adanya. Bentuk itu bukan chat-id WAHA yang sah, jadi pesan
    // apa pun ke tamu mustahil terkirim, dan nomor yang sama tak akan cocok
    // dengan catatan mana pun yang tersimpan dalam bentuk `628…`.
    //
    // Panjangnya pun tak lagi ditebak dari `len() < 6`: pemeriksaan itu
    // meloloskan "123456" sebagai nomor HP yang sah.
    let Some(phone) = crate::models::normalisasi_hp(phone) else {
        bail_user!("{}", crate::models::pesan_hp_tidak_sah());
    };
    let g = PendingGuest {
        name: name.to_string(),
        phone,
        purpose: purpose.trim().to_string(),
    };
    let nomor = g.phone.clone();
    let nama = g.name.clone();
    let json = serde_json::to_string(&g)?;

    // Sudah punya kode yang masih hidup? Pakai yang itu — lihat [`hp_key`].
    // Isinya tetap DIPERBARUI: nama/keperluan yang baru diketik yang benar.
    let kode_lama: Option<String> = redis.get(hp_key(&nomor)).await.unwrap_or(None);
    let (code, kode_baru) = match kode_lama {
        Some(c) => {
            let _: () = redis.set_ex(code_key(&c), &json, CODE_TTL).await?;
            (c, false)
        }
        None => {
            let mut dibuat = None;
            for _ in 0..12 {
                let c = gen_code();
                // NX: baru dianggap milik kita bila benar-benar belum dipakai.
                // Versi lama memeriksa EXISTS lalu SET terpisah — dua tamu yang
                // mendaftar pada detik yang sama bisa sama-sama lolos
                // pemeriksaan, dan yang kedua menimpa data yang pertama.
                let ambil: Option<bool> = redis
                    .set_options(
                        code_key(&c),
                        &json,
                        redis::SetOptions::default()
                            .conditional_set(redis::ExistenceCheck::NX)
                            .with_expiration(redis::SetExpiry::EX(CODE_TTL)),
                    )
                    .await
                    .unwrap_or(None);
                if ambil.is_some() {
                    dibuat = Some(c);
                    break;
                }
            }
            let Some(c) = dibuat else {
                bail_user!("Gagal membuat kode unik, coba lagi.");
            };
            (c, true)
        }
    };
    let _: () = redis.set_ex(hp_key(&nomor), &code, CODE_TTL).await?;

    // ── PENGIRIMAN WA ADALAH SYARAT, BUKAN PELENGKAP ─────────────────────────
    // Kode ini tak pernah tampil di layar, jadi WhatsApp satu-satunya jalannya
    // sampai. Gagal kirim = tamu tak punya kode, dan itu HARUS jadi galat yang
    // terlihat — bukan halaman "menunggu mesin" yang tak akan pernah berubah.
    let terkirim = kirim_kode_wa(redis, http, waha, &nomor, &nama, &code).await;
    match terkirim {
        Kirim::Terkirim => {}
        // Kode lama + baru saja dikirim → tak apa-apa, pesannya sudah ada di
        // WhatsApp tamu. Yang salah justru mengirim ulang tanpa henti.
        Kirim::DitahanBatasLaju if !kode_baru => {}
        Kirim::DitahanBatasLaju => {
            bail_user!(
                "Kode untuk nomor ini baru saja dikirim. Periksa WhatsApp Anda, \
                 atau coba lagi beberapa menit."
            );
        }
        Kirim::Gagal => {
            // Kode baru yang tak sampai ke siapa pun jangan ditinggalkan hidup
            // 12 jam — ia hanya menghalangi percobaan berikutnya lewat `hp_key`.
            if kode_baru {
                let _: () = redis.del(code_key(&code)).await.unwrap_or(());
                let _: () = redis.del(hp_key(&nomor)).await.unwrap_or(());
            }
            bail_user!(
                "Kode gagal dikirim ke WhatsApp {}. Pastikan nomornya benar dan \
                 aktif di WhatsApp, lalu coba lagi.",
                super::ganti_nomor::samarkan(&nomor)
            );
        }
    }

    let ticket = gen_tiket();
    let _: () = redis.set_ex(tiket_key(&ticket), &code, CODE_TTL).await?;
    Ok(GuestTicket {
        ticket,
        tujuan: super::ganti_nomor::samarkan(&nomor),
        kode_baru,
    })
}

/// Hasil percobaan kirim WA — tiga keadaan yang perlakuannya berbeda.
enum Kirim {
    Terkirim,
    DitahanBatasLaju,
    Gagal,
}

/// Kirim kode check-in ke WhatsApp tamu.
async fn kirim_kode_wa(
    redis: &mut ConnectionManager,
    http: &reqwest::Client,
    waha: &crate::config::WahaConfig,
    phone: &str,
    nama: &str,
    code: &str,
) -> Kirim {
    // Batas laju per NOMOR — lihat [`WA_JEDA_DETIK`] untuk kenapa endpoint
    // publik ini tak boleh mengirim tanpa rem.
    let boleh: Option<bool> = redis
        .set_options(
            format!("tamu:wa:{phone}"),
            1i32,
            redis::SetOptions::default()
                .conditional_set(redis::ExistenceCheck::NX)
                .with_expiration(redis::SetExpiry::EX(WA_JEDA_DETIK)),
        )
        .await
        .unwrap_or(None);
    if boleh.is_none() {
        tracing::info!("buku tamu: WA ke {phone} ditahan batas laju");
        return Kirim::DitahanBatasLaju;
    }

    let pesan = format!(
        "🕌 *Buku Tamu PPM Al-Faqih Mandiri*\n\nAssalamu'alaikum {nama},\n\
         Kode check-in Anda: *{code}*\n\n\
         Ketik kode ini di mesin buku tamu di gerbang, lalu tatap kamera. \
         Berlaku hari ini."
    );
    match crate::service::registration::send_wa_text(http, waha, phone, &pesan).await {
        Ok(()) => Kirim::Terkirim,
        Err(e) => {
            // Tak menggagalkan pendaftaran, tapi juga tak didiamkan: bila WAHA
            // mati, ini salah satu tempat pertama yang menunjukkannya.
            tracing::warn!("buku tamu: WA kode gagal ke {phone}: {e:#}");
            crate::service::telegram::report_background_error(
                "Buku tamu: WA gagal",
                format!("Tujuan {phone}: {e:#}"),
            );
            // Jatah batas laju dikembalikan: percobaan yang GAGAL tak boleh
            // menghabiskan jendela 10 menit milik percobaan berikutnya.
            let _: () = redis.del(format!("tamu:wa:{phone}")).await.unwrap_or(());
            Kirim::Gagal
        }
    }
}

/// Cari kode → data tamu, lalu HAPUS kode (sekali pakai). None = tak ada/kadaluarsa.
pub async fn consume_guest(
    redis: &mut ConnectionManager,
    code: &str,
) -> Result<Option<PendingGuest>> {
    // GETDEL, bukan GET lalu DEL: kode tamu sekali pakai, dan dua mesin yang
    // memindai kode yang sama pada saat bersamaan sama-sama lolos GET sebelum
    // salah satunya sempat DEL — keduanya lalu tercatat check-in dengan kode
    // yang sama. GETDEL menyatukan ambil-dan-hapus jadi satu operasi atomik di
    // sisi Redis, jadi hanya satu pemanggil yang bisa menang.
    let json: Option<String> = redis::cmd("GETDEL")
        .arg(code_key(code))
        .query_async(redis)
        .await?;
    let Some(json) = json else {
        return Ok(None);
    };
    let g: PendingGuest = serde_json::from_str(&json)?;
    // Lepas juga penanda "nomor ini punya kode aktif". Tanpa ini, tamu yang
    // datang DUA KALI dalam hari yang sama dikirimi kode yang sudah dipakai
    // (dan sudah dihapus di atas) — mesin gerbang akan menolaknya, dan tak ada
    // yang bisa ia lakukan sampai penanda itu kedaluwarsa 12 jam kemudian.
    let _: () = redis.del(hp_key(&g.phone)).await.unwrap_or(());
    Ok(Some(g))
}

/// Tandai kode sukses check-in (untuk polling HP tamu).
pub async fn mark_done(
    redis: &mut ConnectionManager,
    code: &str,
    checkin: &GuestCheckin,
) -> Result<()> {
    let json = serde_json::to_string(checkin)?;
    let _: () = redis.set_ex(done_key(code), json, DONE_TTL).await?;
    Ok(())
}

/// Status untuk halaman /tamu: Some = sudah di-check-in mesin (tampil ✅).
pub async fn check_status(
    redis: &mut ConnectionManager,
    ticket: &str,
) -> Result<Option<GuestCheckin>> {
    // Browser tamu memegang TIKET, bukan kode. Penerjemahannya di sini, di sisi
    // server — sehingga halaman bisa menunggu konfirmasi mesin tanpa pernah
    // mengetahui kode yang membuktikan ia pemilik nomornya.
    let code: Option<String> = redis.get(tiket_key(ticket)).await?;
    let Some(code) = code else {
        return Ok(None);
    };
    let json: Option<String> = redis.get(done_key(&code)).await?;
    match json {
        Some(j) => Ok(Some(serde_json::from_str(&j)?)),
        None => Ok(None),
    }
}

// ── Layar penjaga: tinjau kunjungan tamu (migrasi 83) ────────────────────────

/// Daftar kunjungan tamu untuk penjaga.
///
/// `hanya_belum` menyisakan yang belum diperiksa — itu pekerjaan penjaga.
/// Riwayat lengkap tetap bisa dibuka untuk menelusuri kunjungan lama.
pub const TAMU_PER_PAGE: i64 = 20;

/// Rentang waktu → (batas paling awal, label untuk layar).
///
/// "Semester ini" dibaca dari tabel semester akademik (migrasi 40), bukan
/// ditebak enam bulan ke belakang: pondok yang memundurkan awal semester akan
/// melihat angka yang salah sepanjang periode itu, dan tak ada yang menyadari
/// karena angkanya tetap tampak masuk akal.
async fn batas_rentang(
    pool: &deadpool_postgres::Pool,
    rentang: &str,
) -> (Option<chrono::DateTime<chrono::Utc>>, String) {
    match rentang {
        "hari_ini" => (Some(super::fmt::days_ago_wib(0)), "Hari ini".into()),
        "7" => (Some(super::fmt::days_ago_wib(6)), "7 hari terakhir".into()),
        "30" => (Some(super::fmt::days_ago_wib(29)), "30 hari terakhir".into()),
        "semester" => match super::santri::current_semester(pool).await {
            Ok((mulai, label)) => (Some(mulai), label),
            Err(_) => (None, "Semua".into()),
        },
        // Termasuk "semua" dan nilai tak dikenal: JANGAN diam-diam menyaring
        // sesuatu yang tak diminta.
        _ => (None, "Semua".into()),
    }
}

fn baris_tamu(g: crate::repository::KunjunganTamu) -> crate::models::TamuMasukItem {
    crate::models::TamuMasukItem {
        id: g.id,
        name: g.name,
        phone: g.phone,
        purpose: g.purpose,
        face_url: g.face_url.unwrap_or_default(),
        waktu_label: super::fmt::fmt_when(g.checked_in_at),
        diperiksa: g.verified_at.is_some(),
        diperiksa_oleh: g.verified_by_name.unwrap_or_default(),
        catatan: g.verify_note,
    }
}

pub async fn tamu_masuk(
    pool: &deadpool_postgres::Pool,
    hanya_belum: bool,
    rentang: &str,
) -> Result<crate::models::TamuMasukData> {
    let (sejak, rentang_label) = batas_rentang(pool, rentang).await;
    let (rows, total, belum) = tokio::join!(
        crate::repository::list_kunjungan_tamu(pool, hanya_belum, sejak, TAMU_PER_PAGE, 0),
        crate::repository::count_kunjungan_tamu(pool, hanya_belum, sejak),
        // Dihitung terpisah dari daftar: saat riwayat lengkap dibuka, angka
        // "menunggu diperiksa" harus tetap menyebut yang menunggu — bukan
        // jumlah baris yang kebetulan sedang tampil.
        crate::repository::count_kunjungan_tamu(pool, true, sejak),
    );
    Ok(crate::models::TamuMasukData {
        belum_diperiksa: belum?,
        total: total?,
        rentang_label,
        items: rows?.into_iter().map(baris_tamu).collect(),
    })
}

/// Halaman berikutnya (gulir-tak-berujung).
pub async fn tamu_masuk_page(
    pool: &deadpool_postgres::Pool,
    hanya_belum: bool,
    rentang: &str,
    offset: i64,
) -> Result<Vec<crate::models::TamuMasukItem>> {
    let (sejak, _) = batas_rentang(pool, rentang).await;
    let rows = crate::repository::list_kunjungan_tamu(
        pool,
        hanya_belum,
        sejak,
        TAMU_PER_PAGE,
        offset.max(0),
    )
    .await?;
    Ok(rows.into_iter().map(baris_tamu).collect())
}

/// Tandai satu kunjungan sudah diperiksa. Catatan kosong = data cocok.
pub async fn periksa_tamu(
    pool: &deadpool_postgres::Pool,
    visit_id: i64,
    penjaga_id: i64,
    catatan: &str,
) -> Result<()> {
    // Batas panjang catatan: kolomnya TEXT, dan kotak catatan yang tak berbatas
    // adalah tempat orang menempelkan apa saja.
    let catatan: String = catatan.chars().take(300).collect();
    if !crate::repository::periksa_kunjungan_tamu(pool, visit_id, penjaga_id, &catatan).await? {
        bail_user!("Kunjungan ini sudah diperiksa orang lain, atau tidak ditemukan.");
    }
    Ok(())
}
