//! Deteksi tipe berkas dari ISINYA (magic number), bukan dari namanya.
//!
//! Nama berkas dipilih pengunggah, jadi ekstensi bukan bukti apa pun: berkas
//! apa saja bisa dinamai `.pdf`. Sebelumnya seluruh jalur unggah menyimpulkan
//! `content_type` semata-mata dari ekstensi — dan `bills`/`guestbook` bahkan
//! melabeli setiap berkas `image/jpeg` tanpa memeriksa apa pun. Akibatnya
//! berkas tersimpan di penyimpanan objek dengan label yang tak sesuai isinya,
//! lalu disajikan kembali dengan label itu.
//!
//! Yang dicek di sini hanya beberapa byte pertama. Itu bukan jaminan berkasnya
//! utuh atau aman, tapi cukup untuk memastikan label yang kita simpan memang
//! menggambarkan isinya.

/// Tebak MIME dari beberapa byte pertama. `None` = bukan format yang didukung.
///
/// Nilai kembaliannya SENGAJA memakai string yang sama persis dengan tabel
/// `classify()` di tiap handler unggah, supaya perbandingannya cukup `==`.
pub fn sniff(b: &[u8]) -> Option<&'static str> {
    // RIFF dipakai bersama WAV dan WEBP — pembedanya ada di byte 8..12, jadi
    // keduanya harus diperiksa sebelum pola yang lebih pendek.
    if b.len() >= 12 && &b[0..4] == b"RIFF" {
        return match &b[8..12] {
            b"WEBP" => Some("image/webp"),
            b"WAVE" => Some("audio/wav"),
            _ => None,
        };
    }
    // MP4: penanda `ftyp` ada di offset 4, bukan 0.
    if b.len() >= 8 && &b[4..8] == b"ftyp" {
        return Some("video/mp4");
    }
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }
    if b.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some("image/png");
    }
    if b.starts_with(b"GIF87a") || b.starts_with(b"GIF89a") {
        return Some("image/gif");
    }
    if b.starts_with(b"%PDF-") {
        return Some("application/pdf");
    }
    if b.starts_with(b"OggS") {
        return Some("audio/ogg");
    }
    // Matroska/WebM: EBML header.
    if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Some("video/webm");
    }
    // MP3 hadir dalam dua bentuk: berawalan tag ID3, atau langsung frame sync
    // (11 bit pertama menyala). Keduanya lazim di berkas nyata.
    if b.starts_with(b"ID3") {
        return Some("audio/mpeg");
    }
    if b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0 {
        return Some("audio/mpeg");
    }
    None
}

/// Apakah isi `b` benar-benar bertipe `declared`?
///
/// Ketat: tipe yang tak terdeteksi ikut ditolak. Semua format yang diterima
/// aplikasi ini punya tanda tangan yang jelas, jadi "tak terdeteksi" berarti
/// berkasnya memang bukan salah satu dari mereka.
pub fn matches(b: &[u8], declared: &str) -> bool {
    sniff(b) == Some(declared)
}

/// Ekstensi berkas yang lazim untuk sebuah MIME — dipakai saat menyusun kunci
/// objek, supaya nama yang tersimpan mengikuti isi berkas.
pub fn ext_for(mime: &str) -> &'static str {
    match mime {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "application/pdf" => "pdf",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        "audio/ogg" => "ogg",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        _ => "bin",
    }
}

/// Gambar apa pun yang didukung — dipakai jalur yang menerima "foto" tanpa
/// mempersoalkan formatnya (bukti bayar, foto tamu).
pub fn sniff_image(b: &[u8]) -> Option<&'static str> {
    match sniff(b) {
        Some(m) if m.starts_with("image/") => Some(m),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mengenali_format_yang_didukung() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(
            sniff(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]),
            Some("image/png")
        );
        assert_eq!(sniff(b"GIF89a...."), Some("image/gif"));
        assert_eq!(sniff(b"%PDF-1.7"), Some("application/pdf"));
        assert_eq!(sniff(b"OggS\0\0\0\0"), Some("audio/ogg"));
        assert_eq!(sniff(b"ID3\x03\0\0\0"), Some("audio/mpeg"));
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x00]), Some("audio/mpeg"));
        assert_eq!(sniff(&[0x1A, 0x45, 0xDF, 0xA3]), Some("video/webm"));
        assert_eq!(sniff(b"\0\0\0\x20ftypmp42"), Some("video/mp4"));
    }

    /// WAV dan WEBP sama-sama berawalan "RIFF" — yang membedakan byte 8..12.
    /// Tanpa pembedaan ini, berkas WAV bisa lolos sebagai gambar.
    #[test]
    fn membedakan_dua_format_riff() {
        assert_eq!(sniff(b"RIFF\0\0\0\0WEBPVP8 "), Some("image/webp"));
        assert_eq!(sniff(b"RIFF\0\0\0\0WAVEfmt "), Some("audio/wav"));
        assert_eq!(sniff(b"RIFF\0\0\0\0AVI LIST"), None);
    }

    #[test]
    fn menolak_yang_bukan_format_didukung() {
        // Skrip yang dinamai ulang jadi .pdf — persis kasus yang dicegah.
        assert_eq!(sniff(b"#!/bin/sh\nrm -rf /"), None);
        assert_eq!(sniff(b"<?php echo 1; ?>"), None);
        assert_eq!(sniff(b""), None);
        assert_eq!(sniff(&[0xFF]), None);
        assert!(!matches(b"#!/bin/sh", "application/pdf"));
        assert!(matches(b"%PDF-1.4", "application/pdf"));
    }

    #[test]
    fn sniff_image_hanya_meloloskan_gambar() {
        assert_eq!(sniff_image(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"));
        assert_eq!(sniff_image(b"%PDF-1.7"), None);
        assert_eq!(sniff_image(b"ID3\x03\0\0\0"), None);
    }

    #[test]
    fn ekstensi_mengikuti_isi() {
        assert_eq!(ext_for("image/png"), "png");
        assert_eq!(ext_for("video/webm"), "webm");
        assert_eq!(ext_for("entah/apa"), "bin");
    }
}
