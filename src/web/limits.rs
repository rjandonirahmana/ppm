//! web/limits.rs — Batas ukuran unggahan, SATU sumber kebenaran.
//!
//! Tiap unggahan dibatasi di DUA tempat yang harus sepakat:
//!   1. Layer `DefaultBodyLimit` di router (main.rs) — memutus koneksi lebih awal,
//!      sebelum byte-nya sempat menumpuk di memori server.
//!   2. Pemeriksaan di dalam handler — yang menghasilkan pesan berbahasa
//!      Indonesia yang benar-benar dibaca pengguna.
//!
//! Sebelumnya keduanya ditulis terpisah dan TIDAK sepakat: handler memeriksa
//! 10 MB/100 MB, sementara router tak memasang layer sama sekali sehingga
//! berlaku bawaan axum, yaitu 2 MB. Akibatnya batas yang sesungguhnya berlaku
//! adalah 2 MB, pemeriksaan di handler tak pernah tercapai, dan foto kamera HP
//! biasa (3–8 MB) ditolak dengan galat multipart yang tak bisa dimengerti
//! pengguna. Menaruh angkanya di sini membuat ketidaksepakatan seperti itu
//! kelihatan saat membaca kode, bukan saat ada yang gagal mengunggah.
//!
//! `body_limit()` sengaja LEBIH LONGGAR dari batas isi file: pembungkus
//! multipart (garis boundary + header tiap field + nama berkas) ikut terhitung
//! di limit body, jadi berkas yang tepat sebesar batas tetap harus lolos sampai
//! ke handler — supaya penolakannya datang dari kita, dengan kalimat kita.

/// Foto kegiatan galeri, wajah tamu, dan bukti bayar tagihan.
pub const IMAGE_MAX: usize = 10 * 1024 * 1024;

/// Berkas Materials Library (mp3/wav/ogg, pdf, mp4/webm).
pub const MATERIAL_MAX: usize = 100 * 1024 * 1024;

/// Satu potongan audio siaran (~4 detik Opus; jauh di bawah angka ini).
pub const AUDIO_CHUNK_MAX: usize = 2_000_000;

/// Batas body HTTP untuk unggahan sebesar `content_max`: isi berkas + ruang
/// untuk pembungkus multipart (lihat catatan modul).
pub const fn body_limit(content_max: usize) -> usize {
    content_max + 2 * 1024 * 1024
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Batas body WAJIB di atas batas isi — kalau tidak, berkas berukuran tepat
    /// di batas ditolak lapisan transport dan pengguna tak pernah melihat pesan
    /// "maks 10MB" milik kita. Justru kebalikannya (batas body LEBIH KECIL dari
    /// batas isi) yang menjadi bug 2 MB itu.
    #[test]
    fn batas_body_selalu_di_atas_batas_isi() {
        for max in [IMAGE_MAX, MATERIAL_MAX, AUDIO_CHUNK_MAX] {
            assert!(body_limit(max) > max, "body_limit({max}) harus > {max}");
        }
    }

    /// Bawaan axum (`axum-core` DEFAULT_LIMIT) adalah 2 MB. Semua batas di sini
    /// harus melampauinya — kalau tidak, layer di router jadi tak ada gunanya
    /// dan kita kembali ke perilaku lama yang salah.
    #[test]
    fn semua_batas_melampaui_bawaan_axum() {
        const AXUM_DEFAULT: usize = 2 * 1024 * 1024;
        for max in [IMAGE_MAX, MATERIAL_MAX, AUDIO_CHUNK_MAX] {
            assert!(body_limit(max) > AXUM_DEFAULT);
        }
    }
}
