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
    // Keluarga ISO-BMFF: penanda `ftyp` ada di offset 4, bukan 0. MP4, MOV
    // (QuickTime), dan HEIC memakai wadah yang SAMA — pembedanya "major brand"
    // di offset 8..12. Sebelumnya semuanya dijawab `video/mp4`, sehingga foto
    // HEIC dari iPhone tersimpan berlabel video dan rekaman .mov ditolak karena
    // label ekstensinya tak cocok dengan hasil sniff.
    if let Some(brand) = brand_isobmff(b) {
        return Some(match brand {
            // "qt  " = QuickTime murni (kamera iPhone sebelum dikonversi).
            b"qt  " => "video/quicktime",
            b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1" => {
                "image/heic"
            }
            // Wadah ISO-BMFF berisi AUDIO saja. Rekaman suara dari ponsel lazim
            // berbentuk ini (.m4a), dan tanpa dibedakan ia dijawab "video/mp4"
            // — label yang salah untuk berkas yang tak punya gambar sama sekali.
            b"M4A " | b"M4B " => "audio/mp4",
            _ => "video/mp4",
        });
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
    // `%PDF-` DICARI, bukan dituntut ada di byte pertama.
    //
    // Spesifikasi PDF memang menempatkannya di offset 0, tapi pembaca PDF mana
    // pun memindainya di awal berkas — dan berkas nyata sering punya sisipan di
    // depan: BOM UTF-8 dari alat yang menulisnya sebagai teks, baris kosong dari
    // pemindai, atau header yang ditambahkan konverter. Menuntutnya di offset 0
    // membuat berkas yang dibuka normal di semua peramban ditolak di sini dengan
    // "Isi file tidak cocok dengan ekstensinya" — pesan yang tak menuntun ke
    // mana pun.
    //
    // Pencariannya terbatas pada kepala yang memang sudah ada di memori (64
    // byte), dan tanda tangannya lima byte yang sangat khas, jadi peluang salah
    // kenal praktis nol.
    if b.windows(5).any(|w| w == b"%PDF-") {
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
    // Frame sync MP3 dicari SESUDAH melewati padding nol di depan.
    //
    // Sebagian penyunting audio menuliskan blok nol sebelum frame pertama.
    // Berkasnya diputar normal di mana-mana, tapi pemeriksaan yang hanya melihat
    // byte ke-0 menolaknya. Yang dilewati HANYA 0x00 — bukan sembarang byte —
    // supaya ini tetap pemeriksaan, bukan tebakan: berkas acak tak akan lolos
    // hanya karena kebetulan memuat 0xFF di suatu tempat.
    let inti = {
        let mulai = b.iter().position(|&x| x != 0).unwrap_or(b.len());
        &b[mulai..]
    };
    if inti.len() >= 2 && inti[0] == 0xFF && (inti[1] & 0xE0) == 0xE0 {
        return Some("audio/mpeg");
    }
    None
}

/// Major brand berkas ISO-BMFF, bila `b` memang berbentuk itu.
///
/// Kotak `ftyp` SEHARUSNYA yang pertama, tapi berkas nyata kerap didahului
/// kotak `wide`, `free`, atau `skip` — QuickTime menuliskannya, dan berkas
/// hasil kamera lama hampir selalu begitu. Memeriksa offset 4 saja membuat
/// berkas-berkas itu tak dikenali sama sekali, lalu ditolak sebagai "isi tak
/// cocok dengan ekstensinya".
///
/// Penelusurannya dibatasi kepala yang ada di memori dan beberapa kotak saja —
/// cukup untuk melewati pembungkus di depan, tanpa berpura-pura mengurai
/// seluruh berkas.
fn brand_isobmff(b: &[u8]) -> Option<&[u8]> {
    let mut off = 0usize;
    for _ in 0..4 {
        if off + 12 > b.len() {
            return None;
        }
        let jenis = &b[off + 4..off + 8];
        if jenis == b"ftyp" {
            return Some(&b[off + 8..off + 12]);
        }
        // Hanya kotak pembungkus yang boleh dilewati. Kotak lain di depan
        // berarti ini bukan berkas ISO-BMFF yang sedang kita cari.
        if !matches!(jenis, b"wide" | b"free" | b"skip") {
            return None;
        }
        let ukuran =
            u32::from_be_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]]) as usize;
        // Ukuran < 8 tak mungkin (header kotaknya sendiri 8 byte); tanpa
        // penjagaan ini `off` tak maju dan penelusurannya berputar di tempat.
        if ukuran < 8 {
            return None;
        }
        off += ukuran;
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
        "audio/mp4" => "m4a",
        "video/mp4" => "mp4",
        "video/quicktime" => "mov",
        "video/webm" => "webm",
        "image/heic" => "heic",
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

    /// MP4, MOV, dan HEIC berbagi wadah ISO-BMFF (`ftyp` di offset 4); yang
    /// membedakan hanya major brand. Tanpa pembedaan ini rekaman .mov dari
    /// iPhone ditolak (label ekstensi ≠ hasil sniff) dan foto HEIC tersimpan
    /// berlabel video.
    #[test]
    fn membedakan_keluarga_ftyp() {
        assert_eq!(sniff(b"\0\0\0\x14ftypqt  "), Some("video/quicktime"));
        assert_eq!(sniff(b"\0\0\0\x18ftypheic"), Some("image/heic"));
        assert_eq!(sniff(b"\0\0\0\x18ftypmif1"), Some("image/heic"));
        assert_eq!(sniff(b"\0\0\0\x20ftypisom"), Some("video/mp4"));
        // Header terpotong (< 12 byte) tak boleh ditebak sebagai video.
        assert_eq!(sniff(b"\0\0\0\x20ftyp"), None);
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

    // ── Kasus terburuk: berkas SAH yang dulu ditolak ─────────────────────────

    /// PDF dengan BOM UTF-8 di depan. Dibuka normal di semua peramban, tapi
    /// dulu ditolak karena `%PDF-` tak persis di byte pertama.
    #[test]
    fn pdf_dengan_bom_tetap_dikenali() {
        assert_eq!(sniff(b"\xEF\xBB\xBF%PDF-1.4"), Some("application/pdf"));
    }

    /// PDF dengan baris kosong di depan — lazim dari pemindai dan konverter.
    #[test]
    fn pdf_dengan_sisipan_baris_tetap_dikenali() {
        assert_eq!(sniff(b"\r\n\r\n%PDF-1.7\n%..."), Some("application/pdf"));
    }

    /// Tapi pencariannya TERBATAS pada kepala yang ada di memori. Tanda tangan
    /// yang letaknya jauh di dalam berkas bukan PDF — ia cuma teks yang
    /// kebetulan menyebutnya.
    #[test]
    fn pdf_di_luar_kepala_tak_dianggap_pdf() {
        let mut v = vec![b'x'; 200];
        v.extend_from_slice(b"%PDF-1.4");
        // Yang dilihat pemanggil sungguhan hanya 64 byte pertama.
        assert_eq!(sniff(&v[..64]), None);
    }

    /// MP3 dengan padding nol sebelum frame pertama — ditulis sebagian
    /// penyunting audio, diputar normal di mana-mana.
    #[test]
    fn mp3_dengan_padding_nol_tetap_dikenali() {
        assert_eq!(sniff(&[0, 0, 0, 0, 0xFF, 0xFB, 0x90, 0x00]), Some("audio/mpeg"));
    }

    /// Yang dilewati HANYA nol. Sampah acak di depan tetap ditolak — kalau
    /// tidak, pemeriksaan ini berhenti jadi pemeriksaan.
    #[test]
    fn sampah_sebelum_frame_sync_tetap_ditolak() {
        assert_eq!(sniff(&[b'M', b'Z', 0x90, 0x00, 0xFF, 0xFB]), None);
    }

    /// Berkas yang SELURUHNYA nol tak boleh dikenali sebagai apa pun — dan
    /// terutama tak boleh membuat pelewatan padding berjalan melewati ujung.
    #[test]
    fn berkas_penuh_nol_bukan_apa_apa_dan_tak_panik() {
        assert_eq!(sniff(&[0u8; 64]), None);
        assert_eq!(sniff(&[0u8; 1]), None);
    }

    /// MP4/MOV yang didahului kotak `wide` — hampir selalu begitu pada berkas
    /// dari kamera lama dan QuickTime. Dulu tak dikenali sama sekali.
    #[test]
    fn ftyp_di_belakang_kotak_pembungkus_tetap_dikenali() {
        let mut v = Vec::new();
        v.extend_from_slice(&8u32.to_be_bytes());
        v.extend_from_slice(b"wide");
        v.extend_from_slice(&20u32.to_be_bytes());
        v.extend_from_slice(b"ftypqt  ");
        assert_eq!(sniff(&v), Some("video/quicktime"));

        let mut v = Vec::new();
        v.extend_from_slice(&8u32.to_be_bytes());
        v.extend_from_slice(b"free");
        v.extend_from_slice(&24u32.to_be_bytes());
        v.extend_from_slice(b"ftypisom");
        assert_eq!(sniff(&v), Some("video/mp4"));
    }

    /// Wadah ISO-BMFF berisi audio saja (.m4a) — rekaman suara dari ponsel.
    /// Dulu dijawab "video/mp4", label yang salah untuk berkas tanpa gambar.
    #[test]
    fn m4a_dikenali_sebagai_audio_bukan_video() {
        assert_eq!(sniff(b"\0\0\0\x18ftypM4A "), Some("audio/mp4"));
        assert_eq!(ext_for("audio/mp4"), "m4a");
    }

    /// Kotak selain pembungkus di depan berarti ini bukan ISO-BMFF yang dicari
    /// — jangan menelusuri lebih jauh dan jangan menebak.
    #[test]
    fn kotak_asing_di_depan_tidak_ditelusuri() {
        let mut v = Vec::new();
        v.extend_from_slice(&8u32.to_be_bytes());
        v.extend_from_slice(b"mdat");
        v.extend_from_slice(&20u32.to_be_bytes());
        v.extend_from_slice(b"ftypisom");
        assert_eq!(sniff(&v), None);
    }

    /// Ukuran kotak 0 atau < 8 adalah data rusak — dan tanpa penjagaan ia
    /// membuat penelusuran tak pernah maju. Harus berhenti, bukan menggantung.
    #[test]
    fn ukuran_kotak_rusak_tak_membuat_gantung() {
        let mut v = Vec::new();
        v.extend_from_slice(&0u32.to_be_bytes()); // ukuran 0
        v.extend_from_slice(b"wide");
        v.extend_from_slice(&20u32.to_be_bytes());
        v.extend_from_slice(b"ftypisom");
        assert_eq!(sniff(&v), None);

        let mut v = Vec::new();
        v.extend_from_slice(&3u32.to_be_bytes()); // ukuran < header
        v.extend_from_slice(b"free");
        v.extend_from_slice(&[0u8; 16]);
        assert_eq!(sniff(&v), None);
    }

    /// Ukuran kotak raksasa menunjuk jauh melewati kepala — berhenti, jangan
    /// mengindeks di luar batas.
    ///
    /// Ukurannya sengaja `0x7FFF_FFFF`, bukan `u32::MAX`: `u32::MAX` menuliskan
    /// `FF FF` sebagai dua byte pertama, dan itu frame-sync MP3 yang SAH — jadi
    /// berkasnya lolos di cabang MP3 di bawah dan uji ini akan menguji hal lain
    /// dari yang dimaksudkan. Tanda tangan MP3 memang selemah itu; yang menahan
    /// berkas asing bukan ia sendirian, melainkan pasangannya dengan ekstensi
    /// yang diminta pengunggah (lihat `materials::classify`).
    #[test]
    fn ukuran_kotak_raksasa_tak_keluar_batas() {
        let mut v = Vec::new();
        v.extend_from_slice(&0x7FFF_FFFFu32.to_be_bytes());
        v.extend_from_slice(b"wide");
        v.extend_from_slice(&[0u8; 32]);
        assert_eq!(sniff(&v), None);
    }

    /// Penelusuran kotak DIBATASI jumlahnya. Berkas yang isinya rantai kotak
    /// pembungkus tanpa akhir tak boleh membuatnya berputar lama-lama.
    #[test]
    fn rantai_pembungkus_panjang_berhenti() {
        let mut v = Vec::new();
        for _ in 0..10 {
            v.extend_from_slice(&8u32.to_be_bytes());
            v.extend_from_slice(b"free");
        }
        assert_eq!(sniff(&v), None);
    }

    /// Masukan terpotong di SETIAP batas panjang yang menentukan cabang. Yang
    /// diuji bukan jawabannya, melainkan bahwa tak satu pun memanik.
    #[test]
    fn masukan_terpotong_tak_pernah_memanik() {
        let contoh: &[&[u8]] = &[
            b"", b"R", b"RI", b"RIF", b"RIFF", b"RIFF\0\0\0\0", b"RIFF\0\0\0\0WEB",
            b"\0", b"\0\0", b"\0\0\0", b"\0\0\0\x18", b"\0\0\0\x18f",
            b"\0\0\0\x18ftyp", b"\0\0\0\x18ftypm", b"%", b"%PDF", b"ID", b"ID3",
            b"Ogg", b"GIF87", &[0xFF], &[0x89], &[0x1A, 0x45, 0xDF],
        ];
        for c in contoh {
            let _ = sniff(c); // cukup: tak memanik
        }
    }

    /// Semua berkas yang jelas BUKAN media tetap ditolak sesudah pelonggaran
    /// di atas — itulah syarat pelonggarannya boleh ada.
    #[test]
    fn berkas_berbahaya_tetap_ditolak() {
        assert_eq!(sniff(b"MZ\x90\x00\x03"), None, "executable Windows");
        assert_eq!(sniff(b"\x7FELF\x02\x01\x01"), None, "executable Linux");
        assert_eq!(sniff(b"PK\x03\x04"), None, "zip/docx/xlsx");
        assert_eq!(sniff(b"<!DOCTYPE html>"), None);
        assert_eq!(sniff(b"<svg xmlns=\"http://www.w3.org/2000/svg\">"), None);
        assert_eq!(sniff(b"#!/usr/bin/env bash"), None);
        assert_eq!(sniff(b"BEGIN:VCALENDAR"), None);
    }

    /// `matches` mengikuti `sniff`, jadi pelonggaran di atas tak boleh
    /// membocorkan label yang salah.
    #[test]
    fn label_tetap_menggambarkan_isi() {
        assert!(matches(b"\xEF\xBB\xBF%PDF-1.4", "application/pdf"));
        assert!(!matches(b"\xEF\xBB\xBF%PDF-1.4", "audio/mpeg"));
        assert!(!matches(b"PK\x03\x04", "application/pdf"));
    }

    #[test]
    fn ekstensi_mengikuti_isi() {
        assert_eq!(ext_for("image/png"), "png");
        assert_eq!(ext_for("video/webm"), "webm");
        assert_eq!(ext_for("entah/apa"), "bin");
    }
}
