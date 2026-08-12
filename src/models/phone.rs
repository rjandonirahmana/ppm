//! models/phone.rs — SATU bentuk simpan untuk nomor HP: `628xxxxxxxxx`.
//!
//! ── KENAPA SATU FUNGSI, DI `models` ──────────────────────────────────────────
//! Sebelumnya ada EMPAT penormal yang ditulis terpisah — `service::auth`,
//! `service::permits`, `service::finance`, dan `service::registration` — dan
//! sebuah jalur (buku tamu) yang tak menormalkan sama sekali. Tiga di antaranya
//! identik baris per baris; yang keempat berbeda aturannya. Nomor yang sama
//! karena itu bisa tersimpan dalam bentuk berbeda tergantung pintu masuknya,
//! dan pencarian `find_by_phone` yang membandingkan teks tak akan menemukannya.
//!
//! Letaknya di `models` karena `models` dikompilasi untuk SEMUA target — server
//! memakainya saat menyimpan, dan WASM bisa memakainya untuk memvalidasi
//! sebelum mengirim. `service` tak tersedia di WASM.
//!
//! ── KENAPA `628…`, BUKAN `08…` ATAU `+628…` ──────────────────────────────────
//! Itu bentuk yang langsung dipakai WAHA sebagai chat-id (`628xxx@c.us`), jadi
//! tak ada penerjemahan kedua yang bisa menyimpang. Ia juga murni angka:
//! gampang divalidasi, tak ada spasi atau tanda plus yang membuat nomor sama
//! tersimpan dalam dua bentuk berbeda.
//!
//! ── CACAT YANG DIPERBAIKI DI SINI ────────────────────────────────────────────
//! Penormal lama membuang non-digit lalu memeriksa awalan `0` saja:
//!
//! ```text
//! "+62 0812-3456-7890" → digit: "6208123456789" → tak diawali '0'
//!                      → tersimpan apa adanya: "6208123456789"  ← CACAT
//! ```
//!
//! Bentuk `620…` itu bukan nomor siapa pun. Ia lolos login (dibandingkan dengan
//! dirinya sendiri) tapi menghasilkan chat-id WAHA yang tak sah — jadi OTP,
//! reset sandi, dan pengingat tagihan untuk akun itu gagal terkirim SELAMANYA,
//! tanpa satu pun galat yang terlihat. Menuliskan nomor dengan spasi dan tanda
//! plus adalah kebiasaan yang sangat wajar; hukumannya tak boleh permanen.

/// Panjang nomor Indonesia yang masuk akal, DI LUAR awalan `62`.
/// Operator memakai 9–12 digit setelah kode negara; 8–13 diberi kelonggaran.
const MIN_DIGIT: usize = 8;
const MAX_DIGIT: usize = 13;

/// Normalisasi nomor HP Indonesia ke bentuk simpan `628xxxxxxxxx`.
///
/// `None` = bukan nomor Indonesia yang bisa ditafsirkan. Pemanggil yang perlu
/// menolak dengan pesan sendiri memakai [`pesan_hp_tidak_sah`].
///
/// Semua bentuk yang lazim diketik orang diterima:
///
/// | Ditulis pengguna        | Tersimpan       |
/// |-------------------------|-----------------|
/// | `081234567890`          | `6281234567890` |
/// | `+62 812-3456-7890`     | `6281234567890` |
/// | `62 812 3456 7890`      | `6281234567890` |
/// | `+62 0812 3456 7890`    | `6281234567890` |
/// | `81234567890`           | `6281234567890` |
pub fn normalisasi_hp(input: &str) -> Option<String> {
    let digit: String = input.chars().filter(|c| c.is_ascii_digit()).collect();

    // Urutannya penting: `620…` HARUS diperiksa sebelum `62…`, kalau tidak ia
    // lolos sebagai nomor yang sudah benar — itu persis cacat lama.
    let inti = if let Some(sisa) = digit.strip_prefix("620") {
        // "+62 0812…" — pengguna menulis kode negara DAN angka nol daerah.
        sisa
    } else if let Some(sisa) = digit.strip_prefix("62") {
        sisa
    } else if let Some(sisa) = digit.strip_prefix('0') {
        sisa
    } else {
        // Ditulis tanpa awalan apa pun ("81234…").
        digit.as_str()
    };

    // Nomor seluler Indonesia selalu mulai dari 8 setelah kode negara. Menolak
    // yang lain mencegah nomor rumah dan angka asal-asalan tersimpan sebagai
    // nomor WhatsApp yang tak pernah bisa dihubungi.
    if !inti.starts_with('8') || !(MIN_DIGIT..=MAX_DIGIT).contains(&inti.len()) {
        return None;
    }
    Some(format!("62{inti}"))
}

/// Kalimat penolakan baku — supaya tiap layar tak mengarang bunyinya sendiri.
pub fn pesan_hp_tidak_sah() -> &'static str {
    "Nomor HP tidak dikenali. Tulis nomor seluler Indonesia, mis. 081234567890 \
     atau +62 812-3456-7890."
}

/// Chat-id WAHA untuk sebuah nomor tersimpan.
///
/// Dipisah dari [`normalisasi_hp`] supaya jelas: yang disimpan di basis data
/// adalah nomornya, bukan alamat chat. Kalau suatu saat pengirimnya bukan WAHA
/// lagi, yang berubah cukup fungsi ini.
pub fn chat_id_wa(hp_tersimpan: &str) -> String {
    format!("{hp_tersimpan}@c.us")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semua_bentuk_lazim_jadi_satu() {
        let harapan = Some("6281234567890".to_string());
        assert_eq!(normalisasi_hp("081234567890"), harapan);
        assert_eq!(normalisasi_hp("6281234567890"), harapan);
        assert_eq!(normalisasi_hp("+6281234567890"), harapan);
        assert_eq!(normalisasi_hp("+62 812-3456-7890"), harapan);
        assert_eq!(normalisasi_hp("62 812 3456 7890"), harapan);
        assert_eq!(normalisasi_hp("  0812 3456 7890  "), harapan);
        // Tanpa awalan apa pun.
        assert_eq!(normalisasi_hp("81234567890"), harapan);
    }

    /// Cacat yang membuat OTP & pengingat gagal terkirim selamanya: kode negara
    /// DAN angka nol daerah ditulis bersamaan. Dulu tersimpan `620812…`.
    #[test]
    fn kode_negara_plus_nol_daerah_tidak_lagi_cacat() {
        assert_eq!(normalisasi_hp("+62 0812-3456-7890"), Some("6281234567890".into()));
        assert_eq!(normalisasi_hp("620812345678"), Some("62812345678".into()));
        // Yang penting: hasilnya TAK PERNAH berawalan "620".
        for masukan in ["+62 0812 3456 789", "62081234567", "0812345678"] {
            let h = normalisasi_hp(masukan).expect("harus sah");
            assert!(!h.starts_with("620"), "{masukan} → {h} masih cacat");
        }
    }

    /// Menormalkan hasil normalisasi tak boleh mengubah apa pun — kalau tidak,
    /// nomor yang lewat dua pintu masuk berakhir dalam dua bentuk.
    #[test]
    fn idempoten() {
        let sekali = normalisasi_hp("0812 3456 7890").unwrap();
        assert_eq!(normalisasi_hp(&sekali), Some(sekali.clone()));
    }

    #[test]
    fn menolak_yang_bukan_nomor_seluler() {
        // Nomor rumah (bukan diawali 8 setelah kode negara).
        assert_eq!(normalisasi_hp("0217654321"), None);
        assert_eq!(normalisasi_hp("+62 21 765 4321"), None);
        // Terlalu pendek / terlalu panjang.
        assert_eq!(normalisasi_hp("0812"), None);
        assert_eq!(normalisasi_hp("0812345678901234567"), None);
        // Bukan angka sama sekali.
        assert_eq!(normalisasi_hp(""), None);
        assert_eq!(normalisasi_hp("bukan nomor"), None);
    }

    #[test]
    fn chat_id_dibentuk_dari_nomor_tersimpan() {
        let hp = normalisasi_hp("081234567890").unwrap();
        assert_eq!(chat_id_wa(&hp), "6281234567890@c.us");
    }
}
