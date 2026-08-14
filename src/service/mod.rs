//! service — Logika bisnis (server-only), dipisah per-domain.
//! Lapisan: handler (web/api, device_api) → service → repository → Postgres.

/// Penolakan yang disebabkan INPUT atau PERBUATAN PENGGUNA — validasi gagal,
/// aturan bisnis tak terpenuhi, data tak ditemukan. BUKAN kerusakan server.
///
/// Kenapa perlu tipe sendiri: `web::api::err` mengirim alarm Telegram ke admin
/// untuk setiap galat yang lewat. Sebelum ada penanda ini, semua galat terlihat
/// sama, sehingga "Judul materi wajib diisi." dan "Nomor HP atau kata sandi
/// salah." ikut membangunkan admin — padahal itu peristiwa sehari-hari, bukan
/// insiden. Kebisingan seperti itu justru MELEMAHKAN pemantauan: kalau alarm
/// berbunyi ratusan kali sehari untuk salah ketik, alarm yang benar-benar
/// penting (Postgres mati, RustFS tak terjangkau) ikut terabaikan.
///
/// Aturannya: `bail_user!` untuk yang salah pada PENGGUNA, `bail!`/`?` untuk
/// yang salah pada SISTEM. Keduanya sama-sama sampai ke pengguna dengan pesan
/// yang sama; bedanya hanya pada apakah admin dipanggil.
#[derive(Debug)]
pub struct UserError(pub String);

impl std::fmt::Display for UserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for UserError {}

/// `bail!` versi pengguna: hentikan dengan pesan yang memang ditujukan untuk
/// dibaca pengguna, tanpa memicu alarm Telegram. Lihat [`UserError`].
#[macro_export]
macro_rules! bail_user {
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        return ::core::result::Result::Err(::anyhow::Error::new(
            $crate::service::UserError(::std::format!($fmt $(, $arg)*)),
        ))
    };
    ($e:expr $(,)?) => {
        return ::core::result::Result::Err(::anyhow::Error::new(
            $crate::service::UserError(::std::string::ToString::to_string(&$e)),
        ))
    };
}

pub mod admin;
pub mod attendance;
pub mod auth;
pub mod books;
pub mod calendar;
pub mod dashboard;
pub mod enrollment;
pub mod export;
pub mod finance;
pub mod ganti_nomor;
pub mod fmt;
pub mod gate;
pub mod guest;
pub mod hafalan;
pub mod ics;
pub mod kelas;
pub mod laporan;
pub mod materials;
pub mod parent;
pub mod permits;
pub mod recording;
pub mod rekap;
pub mod registration;
pub mod santri;
pub mod semester;
pub mod server;
pub mod sessions;
pub mod storage;
pub mod telegram;
