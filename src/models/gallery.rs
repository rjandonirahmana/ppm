//! models/gallery.rs — Foto kegiatan (galeri beranda, migrasi 34). Shared
//! (SSR + hydrate) karena jadi tipe balikan server fn.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActivityPhoto {
    pub id: i64,
    pub url: String,
    pub caption: String,
    pub sort_order: i32,
    /// Titik fokus horizontal 0..1 (0.5 = tengah). Lihat migrasi 54.
    pub focus_x: f32,
    /// Titik fokus vertikal 0..1 (0.5 = tengah).
    pub focus_y: f32,
    /// Perbesaran 1..3 (1 = apa adanya).
    pub zoom: f32,
    /// `"cover"` (penuhi bingkai, terpotong) atau `"contain"` (foto utuh).
    /// Lihat [`PhotoFit`] dan migrasi 55.
    pub fit: String,
}

/// Cara foto mengisi bingkainya.
///
/// Dua kebutuhan yang tak bisa dipenuhi sekaligus: kartu galeri yang rapi
/// menuntut semua foto berukuran sama (maka dipotong), sedangkan sebagian foto
/// justru tak boleh kehilangan apa pun. Karena itu pilihannya per-foto, bukan
/// satu aturan untuk seluruh galeri.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhotoFit {
    /// Penuhi bingkai; sisi yang lebih panjang terpotong. Titik fokus menentukan
    /// bagian mana yang dipertahankan.
    Cover,
    /// Tampilkan foto UTUH; ruang sisa diisi versi buram foto itu sendiri.
    Contain,
}

impl PhotoFit {
    pub fn from_str(s: &str) -> Self {
        if s == "contain" {
            Self::Contain
        } else {
            Self::Cover
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Contain => "contain",
        }
    }

    /// Apakah mode ini menyisakan ruang kosong yang perlu diisi latar buram.
    pub fn needs_backdrop(self) -> bool {
        matches!(self, Self::Contain)
    }
}

impl ActivityPhoto {
    pub fn fit(&self) -> PhotoFit {
        PhotoFit::from_str(&self.fit)
    }

    /// Gaya CSS untuk elemen `<img>` foto: mode isi + titik fokus + perbesaran.
    ///
    /// SATU sumber untuk semua tempat foto kegiatan tampil (beranda publik 3:4,
    /// grid pengelola 1:1, pratinjau editor). Rasio bingkainya boleh berbeda —
    /// justru itu gunanya menyimpan titik fokus alih-alih hasil potongan: nilai
    /// yang sama menghasilkan bidikan yang benar di rasio mana pun.
    pub fn frame_style(&self) -> String {
        frame_style_of(self.focus_x, self.focus_y, self.zoom, self.fit())
    }
}

/// Bentuk bebas dari [`ActivityPhoto::frame_style`] — dipakai editor galeri yang
/// menyusun gayanya secara reaktif dari nilai yang sedang digeser, sebelum ada
/// `ActivityPhoto` yang tersimpan.
///
/// `transform-origin` disamakan dengan titik fokus supaya memperbesar menarik
/// gambar KE ARAH titik itu, bukan ke tengah bingkai — kalau tidak, menggeser
/// lalu memperbesar akan melempar bagian yang barusan dipilih ke luar bingkai.
pub fn frame_style_of(focus_x: f32, focus_y: f32, zoom: f32, fit: PhotoFit) -> String {
    let (x, y) = (focus_x * 100.0, focus_y * 100.0);
    format!(
        "width:100%;height:100%;object-fit:{f};\
         object-position:{x:.2}% {y:.2}%;\
         transform:scale({zoom:.3});transform-origin:{x:.2}% {y:.2}%;",
        f = fit.as_str(),
    )
}

/// Gaya latar buram yang mengisi ruang sisa saat mode `contain`.
///
/// Memakai foto ITU SENDIRI yang diperbesar & diburamkan, bukan blok abu-abu:
/// kartunya tetap terlihat penuh dan warnanya menyatu dengan fotonya, sehingga
/// foto tegak di antara foto lanskap tidak tampak seperti kesalahan tata letak.
pub const BACKDROP_STYLE: &str =
    "position:absolute;inset:0;width:100%;height:100%;object-fit:cover;\
     filter:blur(18px) saturate(1.2);transform:scale(1.15);";

/// Nilai bawaan titik fokus & zoom — tengah, tanpa perbesaran. Sama dengan
/// perilaku sebelum migrasi 54, jadi foto lama tampil persis seperti dulu.
pub const FOCUS_DEFAULT: (f32, f32, f32) = (0.5, 0.5, 1.0);

/// Rapikan nilai dari klien ke rentang yang dijamin `CHECK` migrasi 54 & 55.
/// Dipakai di sisi server sebelum menyimpan — batas di database adalah jaring
/// pengaman terakhir, bukan tempat memvalidasi masukan pengguna.
pub fn clamp_focus(focus_x: f32, focus_y: f32, zoom: f32) -> (f32, f32, f32) {
    // NaN tak tertangkap oleh `clamp` (ia panik pada NaN), jadi dibereskan dulu.
    let fix = |v: f32, lo: f32, hi: f32, fallback: f32| {
        if v.is_finite() {
            v.clamp(lo, hi)
        } else {
            fallback
        }
    };
    (
        fix(focus_x, 0.0, 1.0, 0.5),
        fix(focus_y, 0.0, 1.0, 0.5),
        fix(zoom, 1.0, 3.0, 1.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nilai tak dikenal HARUS jatuh ke `cover` — itu perilaku lama, dan bingkai
    /// yang terpotong jauh lebih tidak mengagetkan daripada tata letak berubah
    /// sendiri karena satu baris data yang aneh.
    #[test]
    fn fit_tak_dikenal_jadi_cover() {
        assert_eq!(PhotoFit::from_str("contain"), PhotoFit::Contain);
        assert_eq!(PhotoFit::from_str("cover"), PhotoFit::Cover);
        assert_eq!(PhotoFit::from_str(""), PhotoFit::Cover);
        assert_eq!(PhotoFit::from_str("COVER"), PhotoFit::Cover);
        assert_eq!(PhotoFit::from_str("bulat"), PhotoFit::Cover);
    }

    /// Latar buram hanya perlu saat foto TIDAK memenuhi bingkai.
    #[test]
    fn backdrop_hanya_untuk_contain() {
        assert!(PhotoFit::Contain.needs_backdrop());
        assert!(!PhotoFit::Cover.needs_backdrop());
    }

    /// NaN/tak hingga tak boleh lolos: `f32::clamp` PANIK pada NaN, dan nilai
    /// seperti itu akan menabrak CHECK di tabel.
    #[test]
    fn clamp_menangani_nan_dan_di_luar_rentang() {
        assert_eq!(clamp_focus(f32::NAN, f32::INFINITY, f32::NAN), (0.5, 0.5, 1.0));
        assert_eq!(clamp_focus(-3.0, 9.0, 99.0), (0.0, 1.0, 3.0));
        assert_eq!(clamp_focus(0.25, 0.75, 1.5), (0.25, 0.75, 1.5));
    }

    /// Mode ikut ke gaya CSS — kalau tidak, memilih "muat seluruhnya" tak
    /// berpengaruh apa-apa di layar.
    #[test]
    fn gaya_memuat_mode_yang_dipilih() {
        assert!(frame_style_of(0.5, 0.5, 1.0, PhotoFit::Contain).contains("object-fit:contain"));
        assert!(frame_style_of(0.5, 0.5, 1.0, PhotoFit::Cover).contains("object-fit:cover"));
    }
}

/// Status check-in tamu (buku tamu, migrasi 35) — dipolling halaman /tamu untuk
/// menampilkan ✅ setelah mesin IoT mengonfirmasi kode + wajah.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GuestCheckin {
    pub name: String,
    pub face_url: String,
}
