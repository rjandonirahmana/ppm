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
    /// Bagian halaman depan tempat media ini tampil. Lihat [`MediaCategory`]
    /// dan migrasi 69.
    pub category: String,
    /// `"image"` atau `"video"` — lihat [`MediaKind`] dan migrasi 69.
    pub media_type: String,
}

/// Bagian halaman depan tempat sebuah media galeri tampil (migrasi 69).
///
/// Galeri dulu satu tumpukan tanpa penanda, jadi halaman depan hanya bisa
/// menampilkannya sebagai satu grid. Padahal isinya tiga hal yang muncul di
/// tempat berbeda, dan pengelola perlu bisa mengganti video kepala halaman
/// tanpa menyentuh foto kegiatan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaCategory {
    /// Media yang berjalan di kepala halaman depan (satu yang teratas dipakai).
    VideoUtama,
    /// Foto kegiatan santri — grid "Kegiatan".
    Kegiatan,
    /// Foto sarana pondok — grid "Fasilitas".
    Fasilitas,
}

impl MediaCategory {
    /// Nilai tak dikenal jatuh ke `Kegiatan`: itu kategori bawaan di tabel, dan
    /// media yang salah tempat jauh lebih baik daripada media yang hilang.
    pub fn from_str(s: &str) -> Self {
        match s {
            "video_utama" => Self::VideoUtama,
            "fasilitas" => Self::Fasilitas,
            _ => Self::Kegiatan,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::VideoUtama => "video_utama",
            Self::Kegiatan => "kegiatan",
            Self::Fasilitas => "fasilitas",
        }
    }

    /// Label berbahasa Indonesia untuk tab & judul bagian.
    pub fn label(self) -> &'static str {
        match self {
            Self::VideoUtama => "Video Utama",
            Self::Kegiatan => "Kegiatan",
            Self::Fasilitas => "Fasilitas",
        }
    }

    /// Urutan tab di halaman kelola — sekaligus daftar kategori yang sah.
    pub const ALL: [MediaCategory; 3] = [Self::VideoUtama, Self::Kegiatan, Self::Fasilitas];
}

/// Jenis berkas media. Dipisah dari kategori karena keduanya memang bisa
/// berbeda: pondok yang belum punya rekaman boleh memakai foto sebagai video
/// utama, dan kelak bisa ada video kegiatan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaKind {
    Image,
    Video,
}

impl MediaKind {
    pub fn from_str(s: &str) -> Self {
        if s == "video" {
            Self::Video
        } else {
            Self::Image
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }

    /// Jenis media yang diwakili sebuah MIME unggahan.
    pub fn of_mime(mime: &str) -> Self {
        if mime.starts_with("video/") {
            Self::Video
        } else {
            Self::Image
        }
    }
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

    pub fn category(&self) -> MediaCategory {
        MediaCategory::from_str(&self.category)
    }

    pub fn kind(&self) -> MediaKind {
        MediaKind::from_str(&self.media_type)
    }

    pub fn is_video(&self) -> bool {
        self.kind() == MediaKind::Video
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

    /// Kategori tak dikenal HARUS jatuh ke `kegiatan` — itu DEFAULT kolomnya
    /// (migrasi 69). Kalau tidak, satu baris aneh bisa mengosongkan grid
    /// kegiatan sekaligus muncul sebagai video kepala halaman.
    #[test]
    fn kategori_tak_dikenal_jadi_kegiatan() {
        assert_eq!(MediaCategory::from_str("video_utama"), MediaCategory::VideoUtama);
        assert_eq!(MediaCategory::from_str("fasilitas"), MediaCategory::Fasilitas);
        assert_eq!(MediaCategory::from_str("kegiatan"), MediaCategory::Kegiatan);
        assert_eq!(MediaCategory::from_str(""), MediaCategory::Kegiatan);
        assert_eq!(MediaCategory::from_str("VIDEO_UTAMA"), MediaCategory::Kegiatan);
    }

    /// Nilai `as_str` WAJIB lolos CHECK migrasi 69, jadi bolak-balik harus utuh.
    #[test]
    fn kategori_bolak_balik_utuh() {
        for c in MediaCategory::ALL {
            assert_eq!(MediaCategory::from_str(c.as_str()), c);
        }
    }

    /// Jenis media disimpulkan dari MIME hasil sniff isi berkas, bukan dari
    /// ekstensi nama — jadi pemetaannya harus mengikuti prefix MIME.
    #[test]
    fn jenis_media_dari_mime() {
        assert_eq!(MediaKind::of_mime("video/mp4"), MediaKind::Video);
        assert_eq!(MediaKind::of_mime("video/webm"), MediaKind::Video);
        assert_eq!(MediaKind::of_mime("image/jpeg"), MediaKind::Image);
        assert_eq!(MediaKind::from_str("video"), MediaKind::Video);
        assert_eq!(MediaKind::from_str(""), MediaKind::Image);
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

/// Hasil pendaftaran tamu di /tamu.
///
/// ── KODENYA TIDAK ADA DI SINI, DAN ITU INTINYA ───────────────────────────────
/// Kode check-in hanya dikirim ke WhatsApp nomor yang diketik tamu — tak pernah
/// dikembalikan ke browser. Justru itulah yang membuat nomornya TERBUKTI: siapa
/// pun bisa mengetik nomor orang lain di formulir, tapi hanya pemegang nomor itu
/// yang menerima kodenya, dan tanpa kode ia tak bisa check-in di gerbang.
///
/// Menampilkan kodenya di layar — seperti rancangan pertama buku tamu ini —
/// membuat isian nomor HP sekadar hiasan: tamu bisa menulis nomor asal-asalan,
/// atau nomor orang lain, dan tetap masuk. Untuk catatan siapa-masuk-pondok,
/// nomor yang tak bisa dihubungi sama saja dengan tak ada nomor.
///
/// Yang dibawa ke layar hanya `ticket` — pengenal acak untuk MENUNGGU
/// konfirmasi mesin (polling), bukan untuk check-in. Mengetahuinya tak memberi
/// kemampuan apa pun di gerbang.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GuestTicket {
    /// Pengenal acak untuk polling status. BUKAN kode check-in.
    pub ticket: String,
    /// Nomor tujuan yang sudah disamarkan (`62812••••890`) — supaya tamu bisa
    /// memastikan ia tak salah ketik, tanpa layar ikut memajang nomor utuh.
    pub tujuan: String,
    /// `false` = kode yang dikirim adalah kode LAMA yang masih berlaku (tamu
    /// mendaftar dua kali dengan nomor sama), jadi layar bisa mengarahkannya ke
    /// pesan WhatsApp yang sudah ada alih-alih menyuruhnya menunggu yang baru.
    pub kode_baru: bool,
}
