//! models/notifikasi.rs — Payload lonceng notifikasi.
//!
//! Teksnya sudah jadi sejak ditulis ke database (lihat `migration/92`), jadi
//! tak ada yang perlu dirangkai ulang di sini — struct ini benar-benar hanya
//! pembawa.

use serde::{Deserialize, Serialize};

/// Jenis kejadian. Dipakai UI untuk memilih ikon & warna, BUKAN untuk menyusun
/// teksnya (teks datang apa adanya dari database).
pub mod jenis {
    /// Ada pengajuan izin masuk — untuk wali kelas & admin.
    pub const IZIN_BARU: &str = "izin_baru";
    /// Pengajuan disetujui — untuk santri.
    pub const IZIN_DISETUJUI: &str = "izin_disetujui";
    /// Pengajuan ditolak — untuk santri.
    pub const IZIN_DITOLAK: &str = "izin_ditolak";
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NotifItem {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub body: String,
    /// Tujuan saat diketuk; kosong = tak ke mana-mana.
    pub link: String,
    pub dibaca: bool,
    /// Sudah diformat relatif ("3 menit lalu"), bukan timestamp mentah —
    /// pemformatan waktu tinggal di service seperti seluruh kode lain.
    pub waktu_label: String,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct NotifData {
    pub items: Vec<NotifItem>,
    /// Jumlah yang belum dibaca — inilah yang menyalakan titik di lonceng.
    pub belum_dibaca: i64,
}
