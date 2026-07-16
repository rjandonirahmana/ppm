//! web/pages — Halaman aplikasi (SSR + hydrate).

mod beranda;
mod dashboard_santri;
mod design_pages;
mod izin;
mod login;
mod menu;
mod not_found;
mod ortu_beranda;
mod ortu_izin;
mod ortu_riwayat;
mod profil;
mod riwayat;
mod sesi;
mod verifikasi_pamong;

pub use beranda::BerandaPage;
pub use dashboard_santri::SantriDashboardPage;
pub use design_pages::*;
pub use izin::IzinPage;
pub use login::LoginPage;
pub use menu::MenuPage;
pub use not_found::NotFoundPage;
pub use ortu_beranda::OrtuBerandaPage;
pub use ortu_izin::OrtuIzinPage;
pub use ortu_riwayat::OrtuRiwayatPage;
pub use profil::ProfilPage;
pub use riwayat::RiwayatPage;
pub use sesi::SesiPage;
pub use verifikasi_pamong::VerifikasiPamongPage;
