//! service/ganti_nomor.rs — Pengguna mengganti nomor WhatsApp-nya sendiri.
//!
//! ── KENAPA HARUS ADA OTP ─────────────────────────────────────────────────────
//! Nomor HP di sini bukan sekadar data kontak: ia IDENTITAS LOGIN (`find_by_phone`
//! di `service::auth`), penerima OTP, penerima reset sandi, dan penerima
//! pengingat tagihan. Membiarkan seseorang menulis nomor apa pun tanpa bukti
//! kepemilikan berarti ia bisa memindahkan akunnya ke nomor orang lain — atau,
//! lebih buruk, salah ketik satu digit lalu terkunci selamanya dari akunnya
//! sendiri karena OTP pemulihan dikirim ke nomor yang tak pernah ia pegang.
//!
//! Karena itu kodenya dikirim ke NOMOR BARU, bukan ke nomor lama. Yang perlu
//! dibuktikan adalah "saya memegang nomor ini", bukan "saya pemilik akun ini" —
//! yang kedua sudah dibuktikan oleh sesi yang sedang berjalan.
//!
//! ── KENAPA MODUL SENDIRI ─────────────────────────────────────────────────────
//! Alurnya mirip `registration`, tapi taruhannya berbeda: registrasi membuat
//! baris BARU dan gagalnya tak merusak apa-apa, sedangkan ini MEMINDAHKAN
//! identitas akun yang sudah hidup. Menumpangkannya di sana berarti satu fungsi
//! melayani dua maksud dengan syarat keamanan yang tak sama. Yang dipakai ulang
//! hanya plumbing-nya (`send_wa_text`, `constant_time_eq`), bukan aturannya.

use anyhow::Result;
use deadpool_postgres::Pool;
use rand::RngExt;
use redis::{aio::ConnectionManager, AsyncCommands};
use serde::{Deserialize, Serialize};

use crate::config::WahaConfig;
use crate::repository as repo;

/// Umur pengajuan ganti nomor. Sama dengan OTP registrasi — cukup untuk
/// membuka WhatsApp dan menyalin kode, terlalu pendek untuk ditebak.
const TTL_DETIK: u64 = 600;

/// Sesudah ini, ajukan ulang dari awal. OTP hanya 6 digit: tanpa batas, seluruh
/// ruang tebakan habis dalam hitungan menit.
const MAKS_PERCOBAAN: u8 = 5;

/// Jeda minimum antar pengiriman. Dihitung dari sisa TTL, jadi tak perlu key
/// kedua yang bisa kedaluwarsa terpisah dan menyisakan celah.
const JEDA_KIRIM_ULANG: i64 = 60;

#[derive(Serialize, Deserialize)]
struct PengajuanNomor {
    /// Sudah dinormalkan ke `628…` — lihat `models::phone`.
    nomor_baru: String,
    otp: String,
    percobaan: u8,
}

fn kunci(user_id: i64) -> String {
    format!("ganti-hp:ppm:{user_id}")
}

/// Ringkasan untuk UI: ke nomor mana kode dikirim, dan berapa detik lagi boleh
/// kirim ulang.
pub struct PengajuanTerkirim {
    /// Disamarkan — lihat [`samarkan`].
    pub tujuan: String,
    pub boleh_kirim_ulang_dalam: i64,
}

/// Tampilkan nomor tanpa membocorkannya utuh: `628123••••890`.
///
/// Layar ini bisa terbuka di tempat umum, dan yang dibutuhkan pengguna cuma
/// memastikan ia tak salah ketik — bukan membaca ulang nomornya sendiri.
fn samarkan(nomor: &str) -> String {
    let n: Vec<char> = nomor.chars().collect();
    if n.len() <= 8 {
        return nomor.to_string();
    }
    let depan: String = n[..5].iter().collect();
    let belakang: String = n[n.len() - 3..].iter().collect();
    format!("{depan}••••{belakang}")
}

/// Langkah 1 — kirim kode ke nomor BARU.
///
/// Belum ada satu pun perubahan di Postgres sampai kodenya dicocokkan; selama
/// jendela 10 menit yang ada hanyalah satu key Redis.
pub async fn mulai(
    pool: &Pool,
    redis: &mut ConnectionManager,
    http: &reqwest::Client,
    waha: &WahaConfig,
    user_id: i64,
    nomor_input: &str,
) -> Result<PengajuanTerkirim> {
    let Some(nomor_baru) = crate::models::normalisasi_hp(nomor_input) else {
        bail_user!("{}", crate::models::pesan_hp_tidak_sah());
    };

    // Nomor yang sama dengan sekarang: ditolak lebih awal, bukan diproses lalu
    // "berhasil" tanpa mengubah apa pun. Pengguna yang salah baca nomornya
    // sendiri berhak tahu itu sebelum menunggu WhatsApp yang tak akan berguna.
    let sekarang = repo::phone_of(pool, user_id).await?;
    if sekarang.as_deref() == Some(nomor_baru.as_str()) {
        bail_user!("Itu nomor yang sedang kamu pakai sekarang.");
    }

    // Diperiksa DI SINI supaya pengguna tahu segera — dan diperiksa SEKALI LAGI
    // saat menyimpan, karena di antara keduanya ada jendela 10 menit tempat
    // orang lain bisa mendaftar dengan nomor itu.
    if matches!(repo::find_by_phone(pool, &nomor_baru).await?, Some(id) if id != user_id) {
        bail_user!("Nomor itu sudah dipakai akun lain.");
    }

    let k = kunci(user_id);
    if let Ok(Some(_)) = redis.get::<_, Option<String>>(&k).await {
        let sisa: i64 = redis.ttl(&k).await.unwrap_or(0);
        let sudah_berlalu = TTL_DETIK as i64 - sisa;
        if sudah_berlalu < JEDA_KIRIM_ULANG {
            bail_user!(
                "Kode baru saja dikirim. Tunggu {} detik lagi.",
                JEDA_KIRIM_ULANG - sudah_berlalu
            );
        }
    }

    let otp = format!("{:06}", rand::rng().random_range(100_000..=999_999));
    let pengajuan = PengajuanNomor {
        nomor_baru: nomor_baru.clone(),
        otp: otp.clone(),
        percobaan: 0,
    };
    let _: () = redis
        .set_ex(&k, serde_json::to_string(&pengajuan)?, TTL_DETIK)
        .await
        .map_err(|e| anyhow::anyhow!("Redis SET gagal: {e}"))?;

    let pesan = format!(
        "🔐 *Ganti Nomor AFM SMART*\n\nKode verifikasi: *{otp}*\n\n\
         Masukkan kode ini di halaman Profil untuk memindahkan akunmu ke nomor ini. \
         Berlaku 10 menit.\n\n\
         _Kamu tidak merasa meminta ini? Abaikan pesan ini — akunmu tetap di nomor lama._"
    );
    crate::service::registration::send_wa_text(http, waha, &nomor_baru, &pesan).await?;
    tracing::info!(user_id, "OTP ganti nomor terkirim");

    Ok(PengajuanTerkirim {
        tujuan: samarkan(&nomor_baru),
        boleh_kirim_ulang_dalam: JEDA_KIRIM_ULANG,
    })
}

/// Langkah 2 — cocokkan kode, lalu pindahkan nomornya.
///
/// Return nomor baru (bentuk simpan) supaya pemanggil bisa menandatangani ulang
/// token sesi: nomor ikut di dalam klaim, dan klaim basi akan terus
/// diperbarui ke nomor lama setiap kunjungan.
pub async fn verifikasi(
    pool: &Pool,
    redis: &mut ConnectionManager,
    user_id: i64,
    otp_input: &str,
) -> Result<String> {
    let k = kunci(user_id);
    let Ok(Some(json)) = redis.get::<_, Option<String>>(&k).await else {
        bail_user!("Pengajuan ganti nomor sudah kedaluwarsa. Ulangi dari awal.");
    };
    let Ok(mut pengajuan) = serde_json::from_str::<PengajuanNomor>(&json) else {
        bail_user!("Pengajuan tidak terbaca. Ulangi dari awal.");
    };

    if !crate::service::registration::constant_time_eq(&pengajuan.otp, otp_input.trim()) {
        pengajuan.percobaan = pengajuan.percobaan.saturating_add(1);
        if pengajuan.percobaan >= MAKS_PERCOBAAN {
            let _: () = redis.del(&k).await.unwrap_or(());
            bail_user!("Terlalu banyak percobaan. Ajukan ganti nomor lagi dari awal.");
        }
        // TTL dipertahankan: percobaan yang gagal tak boleh memperpanjang umur
        // pengajuan, kalau tidak seseorang bisa menahannya hidup selamanya.
        let sisa: i64 = redis.ttl(&k).await.unwrap_or(0);
        let _: () = redis
            .set_ex(&k, serde_json::to_string(&pengajuan)?, sisa.max(1) as u64)
            .await
            .unwrap_or(());
        bail_user!(
            "Kode salah. Sisa {} percobaan.",
            MAKS_PERCOBAAN - pengajuan.percobaan
        );
    }

    // Pemeriksaan KEDUA, dan yang ini yang menentukan. Di antara langkah 1 dan
    // 2 ada jendela 10 menit; siapa pun bisa mendaftar dengan nomor itu di
    // sela-selanya. `uq_users_phone` (migrasi 19) tetap menjaga di lapis
    // terakhir, tapi galat constraint mentah bukan kalimat yang pantas dibaca
    // pengguna.
    if matches!(repo::find_by_phone(pool, &pengajuan.nomor_baru).await?, Some(id) if id != user_id)
    {
        let _: () = redis.del(&k).await.unwrap_or(());
        bail_user!("Nomor itu keburu dipakai akun lain. Coba nomor lain.");
    }

    if !repo::set_phone_number(pool, user_id, &pengajuan.nomor_baru).await? {
        bail_user!("Nomor gagal disimpan. Coba lagi sebentar lagi.");
    }
    let _: () = redis.del(&k).await.unwrap_or(());
    tracing::info!(user_id, "nomor WhatsApp diganti");
    Ok(pengajuan.nomor_baru)
}

/// Batalkan pengajuan yang belum diverifikasi — dipakai tombol "Batal" di UI.
///
/// Bukan sekadar merapikan: tanpa ini, pengguna yang salah ketik nomor harus
/// menunggu 10 menit sebelum boleh mengajukan nomor yang benar.
pub async fn batalkan(redis: &mut ConnectionManager, user_id: i64) -> Result<()> {
    let _: () = redis.del(kunci(user_id)).await.unwrap_or(());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::samarkan;

    /// Cukup untuk memastikan tak salah ketik, tak cukup untuk dibaca orang
    /// yang mengintip dari sebelah.
    #[test]
    fn nomor_disamarkan_di_tengah() {
        assert_eq!(samarkan("628123456890"), "62812••••890");
        assert_eq!(samarkan("6281234567"), "62812••••567");
    }

    /// Nomor yang mustahil pendek dibiarkan apa adanya — memotongnya justru
    /// menghasilkan tampilan yang membingungkan, dan bentuk ini toh tak lolos
    /// `normalize_phone`.
    #[test]
    fn nomor_terlalu_pendek_tak_dipotong() {
        assert_eq!(samarkan("62812"), "62812");
        assert_eq!(samarkan(""), "");
    }
}
