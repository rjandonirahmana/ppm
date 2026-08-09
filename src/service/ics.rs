//! service/ics.rs — Langganan kalender (iCalendar/ICS) per pengguna.
//!
//! KENAPA LANGGANAN, BUKAN KIRIM MINGGUAN. Menulis langsung ke Google Calendar
//! seseorang mustahil tanpa OAuth per-orang (lihat catatan di
//! `service/calendar.rs`), dan mengirim tautan "Tambah ke Calendar" satu per
//! satu tak terpakai untuk jadwal sepekan — satu tautan hanya memuat satu
//! acara, sedangkan sepekan berisi belasan sesi.
//!
//! Yang dipakai di sini: satu URL rahasia per pengguna yang menyajikan seluruh
//! jadwalnya sebagai berkas `.ics`. Santri menambahkannya SEKALI di Google
//! Calendar ("Dari URL"), lalu Google sendiri yang menarik ulang secara
//! berkala. Akibatnya jadwalnya tak pernah basi: sesi yang digeser ikut
//! bergeser, dan sesi yang ditandai libur berubah jadi dicoret — tanpa satu pun
//! pesan mingguan yang perlu dikirim, dan tanpa email.
//!
//! CATATAN OPERASIONAL: Google menarik langganan sekitar sekali setiap
//! beberapa jam, bukan seketika. Untuk perubahan mendadak ("sesi sore ini
//! libur"), WhatsApp tetap kanal yang benar.

use chrono::{Datelike, Duration, NaiveDate, NaiveTime, Timelike, Utc};
use deadpool_postgres::Pool;
use sha2::{Digest, Sha256};

use crate::models::SessionUser;
use crate::repository as repo;

/// Rentang yang disajikan: sedikit ke belakang (riwayat pekan lalu masih
/// berguna saat santri menengok ke belakang) dan sepenuh semester ke depan.
const HARI_KE_BELAKANG: i64 = 30;
const HARI_KE_DEPAN: i64 = 120;

/// Batas jumlah acara dalam satu berkas. Jadwal seorang santri untuk 5 bulan
/// berkisar ratusan sesi; angka ini pagar agar satu permintaan tak pernah
/// menghasilkan berkas raksasa.
const MAX_ACARA: i64 = 1500;

/// Sesi tak menyimpan jam selesai (lihat `SessionRow`), jadi durasinya
/// diasumsikan satu jam — sama dengan asumsi tautan "Tambah ke Google Calendar"
/// yang sudah dipakai broadcast WhatsApp, supaya keduanya tak berbeda.
const DURASI_DEFAULT_JAM: i64 = 1;

// ── Token URL ────────────────────────────────────────────────────────────────

/// HMAC-SHA256 (RFC 2104). Ditulis tangan karena crate `hmac` belum ada di
/// proyek ini dan yang dibutuhkan hanya satu fungsi; kebenarannya dijaga uji
/// vektor resmi RFC 4231 di bawah — kriptografi tulis-tangan tanpa uji vektor
/// tak boleh dipercaya.
fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    const BLOK: usize = 64;
    let mut k = [0u8; BLOK];
    if key.len() > BLOK {
        let d = Sha256::digest(key);
        k[..32].copy_from_slice(&d);
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut ipad = [0x36u8; BLOK];
    let mut opad = [0x5cu8; BLOK];
    for i in 0..BLOK {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }
    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(msg);
    let inner = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner);
    outer.finalize().into()
}

/// Token langganan untuk seorang pengguna.
///
/// Diturunkan dari `JWT_SECRET`, jadi tak perlu kolom baru dan tak perlu
/// disimpan di mana pun. Konsekuensinya juga jelas dan disengaja: mengganti
/// `JWT_SECRET` mematikan SEMUA langganan sekaligus — itulah tombol pencabutan
/// massalnya. Untuk mencabut satu orang, kolom versi per-pengguna perlu
/// ditambahkan; belum dibutuhkan.
///
/// 16 byte (32 hex) — 128 bit, jauh di luar jangkauan tebakan, dan cukup pendek
/// untuk URL yang mungkin diketik ulang orang.
pub fn token_langganan(jwt_secret: &str, user_id: i64) -> String {
    let tag = hmac_sha256(jwt_secret.as_bytes(), format!("ics:v1:{user_id}").as_bytes());
    tag[..16].iter().map(|b| format!("{b:02x}")).collect()
}

/// Cocokkan token dari URL. Perbandingan waktu-tetap: pembanding biasa berhenti
/// di byte pertama yang beda, dan selisih waktunya bisa dipakai menebak token
/// karakter demi karakter.
pub fn token_cocok(jwt_secret: &str, user_id: i64, token: &str) -> bool {
    let benar = token_langganan(jwt_secret, user_id);
    if benar.len() != token.len() {
        return false;
    }
    benar
        .bytes()
        .zip(token.bytes())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// Alamat langganan (path + query) untuk seorang pengguna. Origin ditambahkan
/// di layar, karena servernya tak selalu tahu nama domain publiknya sendiri.
pub fn path_langganan(jwt_secret: &str, user_id: i64) -> String {
    format!("/kalender.ics?u={user_id}&t={}", token_langganan(jwt_secret, user_id))
}

// ── Penyusunan berkas ────────────────────────────────────────────────────────

/// Escape nilai TEXT iCalendar (RFC 5545 §3.3.11): backslash, titik koma,
/// koma, dan baris baru punya arti struktural di format ini.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ';' => out.push_str("\\;"),
            ',' => out.push_str("\\,"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            _ => out.push(c),
        }
    }
    out
}

/// Lipat baris pada 75 OKTET (RFC 5545 §3.1) dan akhiri CRLF.
///
/// Batasnya oktet, bukan karakter: nama kelas berbahasa Indonesia bisa memuat
/// karakter multi-byte, dan memotong di tengah salah satunya menghasilkan UTF-8
/// rusak yang membuat sebagian klien menolak SELURUH berkas.
fn lipat(out: &mut String, baris: &str) {
    const BATAS: usize = 73;
    let b = baris.as_bytes();
    let mut mulai = 0;
    let mut pertama = true;
    while mulai < b.len() {
        let sisa = b.len() - mulai;
        let mut ambil = if pertama { BATAS } else { BATAS - 1 }.min(sisa);
        // Mundur ke batas karakter UTF-8 terdekat.
        //
        // URUTAN SYARATNYA PENTING: batas panjang diperiksa SEBELUM byte-nya
        // dibaca. Pada potongan terakhir `mulai + ambil == b.len()`, dan
        // membaca `b[b.len()]` lebih dulu adalah panik indeks — yang berarti
        // setiap permintaan kalender berbalas 500, bukan sekadar lipatan salah.
        while ambil > 0 && mulai + ambil < b.len() && (b[mulai + ambil] & 0xC0) == 0x80 {
            ambil -= 1;
        }
        if ambil == 0 {
            ambil = sisa;
        }
        if !pertama {
            out.push(' ');
        }
        out.push_str(&baris[mulai..mulai + ambil]);
        out.push_str("\r\n");
        mulai += ambil;
        pertama = false;
    }
}

/// Waktu lokal WIB → cap waktu UTC bergaya iCalendar (`20260810T213000Z`).
///
/// Dikonversi ke UTC alih-alih melampirkan blok VTIMEZONE: hasilnya tak
/// mungkin ambigu di klien mana pun, dan berkasnya jauh lebih pendek.
fn utc_stamp(date: NaiveDate, time: NaiveTime) -> String {
    let wib = date.and_time(time);
    let utc = wib - Duration::hours(7);
    format!(
        "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
        utc.year(),
        utc.month(),
        utc.day(),
        utc.hour(),
        utc.minute(),
        utc.second()
    )
}

fn tanggal_polos(d: NaiveDate) -> String {
    format!("{:04}{:02}{:02}", d.year(), d.month(), d.day())
}

/// Susun berkas ICS berisi jadwal `user`.
///
/// Cakupannya mengikuti peran, sama seperti halaman kalender: santri melihat
/// kelas yang diikutinya, orang tua melihat kelas anak-anaknya yang terhubung.
/// Staf memakai jalur santri juga — jadwal yang ia AMPU adalah gagasan lain
/// (dan biasanya kosong di sini), jadi lebih jujur menyajikan kosong daripada
/// diam-diam menyajikan seluruh sesi pondok.
pub async fn bangun_ics(pool: &Pool, jwt_secret: &str, user: &SessionUser) -> anyhow::Result<String> {
    let hari_ini = super::fmt::today_wib();
    let sejak = hari_ini - Duration::days(HARI_KE_BELAKANG);
    let sampai = hari_ini + Duration::days(HARI_KE_DEPAN);

    let rows = if user.role == "parent" {
        repo::sessions_for_parent(pool, user.id, sejak, sampai, MAX_ACARA).await?
    } else {
        repo::sessions_for_student(pool, user.id, sejak, sampai, MAX_ACARA).await?
    };

    // Satu cap waktu untuk seluruh berkas — DTSTAMP menandai "kapan berkas ini
    // disusun", bukan kapan tiap acara dibuat.
    let dtstamp = {
        let n = Utc::now().naive_utc();
        format!(
            "{:04}{:02}{:02}T{:02}{:02}{:02}Z",
            n.year(),
            n.month(),
            n.day(),
            n.hour(),
            n.minute(),
            n.second()
        )
    };

    let mut s = String::with_capacity(rows.len() * 320 + 512);
    lipat(&mut s, "BEGIN:VCALENDAR");
    lipat(&mut s, "VERSION:2.0");
    lipat(&mut s, "PRODID:-//PPM Al-Faqih Mandiri//AFM SMART//ID");
    lipat(&mut s, "CALSCALE:GREGORIAN");
    lipat(&mut s, "METHOD:PUBLISH");
    lipat(&mut s, &format!("X-WR-CALNAME:Jadwal {}", esc(&user.name)));
    lipat(&mut s, "X-WR-TIMEZONE:Asia/Jakarta");
    // Usul frekuensi tarik-ulang. Google memperlakukannya sebagai saran, bukan
    // perintah — disebutkan supaya klien yang menghormatinya tak menarik tiap
    // beberapa menit, dan supaya yang membaca berkas ini tahu itu memang usul.
    lipat(&mut s, "REFRESH-INTERVAL;VALUE=DURATION:PT6H");
    lipat(&mut s, "X-PUBLISHED-TTL:PT6H");

    for r in rows {
        let judul = r.title.clone().unwrap_or_else(|| r.class_name.clone());
        let pengajar = r.teacher.clone().unwrap_or_else(|| "Belum ditentukan".into());

        lipat(&mut s, "BEGIN:VEVENT");
        // UID stabil per SESI. Inilah yang membuat langganan ini memperbarui
        // acara alih-alih menumpuk salinan baru tiap kali ditarik ulang.
        lipat(&mut s, &format!("UID:sesi-{}@afm-smart.ppm", r.id));
        lipat(&mut s, &format!("DTSTAMP:{dtstamp}"));

        match r.start_time {
            Some(mulai) => {
                let selesai = mulai + Duration::hours(DURASI_DEFAULT_JAM);
                lipat(&mut s, &format!("DTSTART:{}", utc_stamp(r.session_date, mulai)));
                // Lewat tengah malam: jam selesai jatuh di tanggal berikutnya.
                let tanggal_selesai = if selesai < mulai {
                    r.session_date.succ_opt().unwrap_or(r.session_date)
                } else {
                    r.session_date
                };
                lipat(&mut s, &format!("DTEND:{}", utc_stamp(tanggal_selesai, selesai)));
            }
            // Sesi ad-hoc tanpa jam → acara sepanjang hari; DTEND eksklusif.
            None => {
                let besok = r.session_date.succ_opt().unwrap_or(r.session_date);
                lipat(&mut s, &format!("DTSTART;VALUE=DATE:{}", tanggal_polos(r.session_date)));
                lipat(&mut s, &format!("DTEND;VALUE=DATE:{}", tanggal_polos(besok)));
            }
        }

        lipat(&mut s, &format!("SUMMARY:{}", esc(&judul)));
        lipat(
            &mut s,
            &format!(
                "DESCRIPTION:{}",
                esc(&format!(
                    "Kelas: {}\nPengajar: {}\n\nJadwal dari AFM SMART.",
                    r.class_name, pengajar
                ))
            ),
        );
        lipat(&mut s, &format!("LOCATION:{}", esc(&r.class_name)));
        // Sesi libur DISERTAKAN sebagai CANCELLED, bukan dibuang: acara yang
        // hilang begitu saja dari langganan tak terlihat oleh santri yang sudah
        // terlanjur mencatatnya, sedangkan yang dicoret jelas terbaca "batal".
        lipat(
            &mut s,
            if r.status == "cancelled" { "STATUS:CANCELLED" } else { "STATUS:CONFIRMED" },
        );
        lipat(&mut s, "END:VEVENT");
    }

    lipat(&mut s, "END:VCALENDAR");
    let _ = jwt_secret;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// Vektor uji resmi RFC 4231 untuk HMAC-SHA-256. Implementasi HMAC yang
    /// ditulis tangan tanpa uji ini tak punya dasar untuk dipercaya.
    #[test]
    fn hmac_cocok_vektor_rfc4231() {
        // Kasus 1: kunci 20 × 0x0b, data "Hi There".
        assert_eq!(
            hex(&hmac_sha256(&[0x0b; 20], b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // Kasus 2: kunci "Jefe".
        assert_eq!(
            hex(&hmac_sha256(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Kasus 3: kunci 20 × 0xaa, data 50 × 0xdd.
        assert_eq!(
            hex(&hmac_sha256(&[0xaa; 20], &[0xdd; 50])),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
        // Kunci LEBIH PANJANG dari blok 64 byte → wajib di-hash dulu.
        assert_eq!(
            hex(&hmac_sha256(&[0xaa; 131], b"Test Using Larger Than Block-Size Key - Hash Key First")),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    /// Token harus berbeda per pengguna dan per secret — kalau tidak, satu URL
    /// yang bocor membuka jadwal semua orang.
    #[test]
    fn token_berbeda_per_pengguna_dan_secret() {
        let a = token_langganan("rahasia", 1);
        let b = token_langganan("rahasia", 2);
        let c = token_langganan("rahasia-lain", 1);
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn token_cocok_hanya_untuk_pemiliknya() {
        let t = token_langganan("rahasia", 7);
        assert!(token_cocok("rahasia", 7, &t));
        assert!(!token_cocok("rahasia", 8, &t));
        assert!(!token_cocok("rahasia-lain", 7, &t));
        assert!(!token_cocok("rahasia", 7, ""));
        assert!(!token_cocok("rahasia", 7, &t[..30]));
    }

    #[test]
    fn escape_menutup_karakter_struktural() {
        assert_eq!(esc("Kelas A; B, C"), "Kelas A\\; B\\, C");
        assert_eq!(esc("baris1\nbaris2"), "baris1\\nbaris2");
        assert_eq!(esc("a\\b"), "a\\\\b");
    }

    /// WIB → UTC = minus 7 jam, termasuk saat mundur melewati tengah malam.
    #[test]
    fn cap_waktu_dikonversi_ke_utc() {
        let d = NaiveDate::from_ymd_opt(2026, 8, 10).unwrap();
        assert_eq!(
            utc_stamp(d, NaiveTime::from_hms_opt(12, 30, 0).unwrap()),
            "20260810T053000Z"
        );
        // 04:40 WIB = 21:40 UTC HARI SEBELUMNYA — kelas subuh, kasus paling
        // sering di pondok ini.
        assert_eq!(
            utc_stamp(d, NaiveTime::from_hms_opt(4, 40, 0).unwrap()),
            "20260809T214000Z"
        );
    }

    /// Baris panjang wajib terlipat, dan lipatannya TIDAK boleh memotong
    /// karakter multi-byte — berkas UTF-8 yang rusak ditolak seluruhnya oleh
    /// sebagian klien kalender.
    #[test]
    fn lipatan_menghormati_batas_utf8() {
        let mut out = String::new();
        let panjang = format!("SUMMARY:{}", "Ngaji Kitab Ta'lim ".repeat(12));
        lipat(&mut out, &panjang);
        assert!(out.ends_with("\r\n"));
        for baris in out.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(baris.len() <= 75, "baris {} oktet: {baris}", baris.len());
        }
        // Isi utuh setelah lipatan dibuka kembali.
        let kembali = out.replace("\r\n ", "").replace("\r\n", "");
        assert_eq!(kembali, panjang);

        let mut multi = String::new();
        let teks = format!("SUMMARY:{}", "Ké".repeat(60));
        lipat(&mut multi, &teks);
        assert_eq!(multi.replace("\r\n ", "").replace("\r\n", ""), teks);
        for baris in multi.split("\r\n").filter(|l| !l.is_empty()) {
            assert!(baris.len() <= 75, "baris {} oktet", baris.len());
        }
    }

    /// Baris PENDEK (mayoritas isi berkas: BEGIN:VEVENT, STATUS, DTSTART…)
    /// harus selamat apa adanya.
    ///
    /// Uji ini ada karena bug nyata: pemeriksa batas UTF-8 membaca
    /// `b[mulai + ambil]` sebelum memastikan indeksnya masih di dalam
    /// rentang — dan pada potongan terakhir indeks itu SELALU sama dengan
    /// panjangnya. Setiap baris, sependek apa pun, memanik.
    #[test]
    fn baris_pendek_tidak_memanik() {
        for teks in ["BEGIN:VEVENT", "STATUS:CANCELLED", "X", "DTSTART:20260810T053000Z"] {
            let mut out = String::new();
            lipat(&mut out, teks);
            assert_eq!(out, format!("{teks}\r\n"));
        }
        // Tepat di batas, satu di bawah, dan satu di atas.
        for n in [72, 73, 74] {
            let mut out = String::new();
            let teks = "a".repeat(n);
            lipat(&mut out, &teks);
            assert_eq!(out.replace("\r\n ", "").replace("\r\n", ""), teks);
        }
    }
}
