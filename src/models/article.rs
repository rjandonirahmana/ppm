//! models/article.rs — Artikel halaman depan (migrasi 69). Shared (SSR +
//! hydrate) karena jadi tipe balikan server fn.

use serde::{Deserialize, Serialize};

/// Satu artikel. `body` ikut dibawa bahkan di daftar: artikel pondok pendek
/// (beberapa paragraf), dan memisahkan query daftar dari query isi hanya
/// menambah satu bulak-balik untuk data yang muat di satu layar.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Article {
    pub id: i64,
    /// Alamat publik: `/artikel/<slug>`. Unik (migrasi 69).
    pub slug: String,
    pub title: String,
    /// Ringkasan untuk kartu di halaman depan.
    pub excerpt: String,
    pub body: String,
    pub cover_url: Option<String>,
    /// Draf (`false`) tak pernah tampil di halaman publik.
    pub published: bool,
    /// Tanggal terbit terformat (`"8 Agustus 2026"`) — diformat di server
    /// supaya klien tak perlu membawa tabel nama bulan sendiri.
    pub created_at: String,
}

impl Article {
    /// Ringkasan yang PASTI ada: kalau pengelola membiarkan kolomnya kosong,
    /// ambil awal isinya. Kartu tanpa ringkasan tampak seperti artikel rusak,
    /// dan memaksa kolom itu wajib hanya memindahkan bebannya ke pengelola.
    pub fn summary(&self) -> String {
        if !self.excerpt.trim().is_empty() {
            return self.excerpt.trim().to_string();
        }
        let body = self.body.trim();
        let cut = body
            .char_indices()
            .nth(160)
            .map(|(i, _)| i)
            .unwrap_or(body.len());
        if cut < body.len() {
            format!("{}…", body[..cut].trim_end())
        } else {
            body.to_string()
        }
    }
}

/// Ubah judul jadi slug URL: huruf kecil, hanya `a-z0-9`, dipisah tanda hubung.
///
/// Dipakai di SERVER saat menyimpan, tapi tinggal di `models` supaya form admin
/// bisa memperlihatkan alamat yang akan terbentuk sambil pengelola mengetik —
/// dua salinan aturan ini pasti akan berbeda cepat atau lambat.
///
/// Judul yang seluruhnya non-ASCII (mis. Arab) menghasilkan slug kosong; itu
/// bukan alamat yang sah, jadi pemanggil harus menyediakan cadangan.
pub fn slugify(title: &str) -> String {
    let mut out = String::with_capacity(title.len());
    let mut prev_dash = true; // true di awal → tanda hubung pembuka tak ikut
    for ch in title.chars() {
        let c = ch.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    // Slug PostgreSQL-nya VARCHAR(160); potong di batas itu, bukan di tengah
    // penyimpanan yang akan menolak dengan galat mentah.
    if out.len() > 160 {
        out.truncate(160);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_hanya_huruf_angka_dan_hubung() {
        assert_eq!(slugify("Kajian Kitab Malam Jumat"), "kajian-kitab-malam-jumat");
        assert_eq!(slugify("  Wisuda 2025!  "), "wisuda-2025");
        assert_eq!(slugify("Al-Qur'an & Hadits"), "al-qur-an-hadits");
    }

    /// Judul tanpa satu pun karakter ASCII alfanumerik tak bisa jadi alamat.
    /// Pemanggil WAJIB menyiapkan cadangan — kalau tidak, seluruh artikel
    /// seperti itu akan bertabrakan di slug kosong yang unik.
    #[test]
    fn judul_non_ascii_menghasilkan_slug_kosong() {
        assert_eq!(slugify("القرآن"), "");
        assert_eq!(slugify("——"), "");
    }

    /// Batas VARCHAR(160) dijaga di sini, bukan diserahkan ke Postgres.
    #[test]
    fn slug_dipotong_di_160() {
        let s = slugify(&"a ".repeat(200));
        assert!(s.len() <= 160, "panjang {}", s.len());
        assert!(!s.ends_with('-'));
    }

    /// Kartu tanpa ringkasan tampak rusak — isinya dipakai sebagai cadangan.
    #[test]
    fn ringkasan_jatuh_ke_awal_isi() {
        let a = |excerpt: &str, body: &str| Article {
            id: 1,
            slug: "x".into(),
            title: "X".into(),
            excerpt: excerpt.into(),
            body: body.into(),
            cover_url: None,
            published: true,
            created_at: String::new(),
        };
        assert_eq!(a("Ringkas", "Isi panjang").summary(), "Ringkas");
        assert_eq!(a("   ", "Isi panjang").summary(), "Isi panjang");
        let panjang = "x".repeat(300);
        let s = a("", &panjang).summary();
        assert!(s.ends_with('…'));
        assert_eq!(s.chars().count(), 161);
    }

    /// Pemotongan harus di batas KARAKTER, bukan byte: judul/isi berbahasa
    /// Indonesia bisa memuat karakter multi-byte, dan mengiris di tengahnya
    /// membuat `String` panik.
    #[test]
    fn ringkasan_aman_untuk_multibyte() {
        let body = "é".repeat(300);
        let a = Article {
            id: 1,
            slug: "x".into(),
            title: "X".into(),
            excerpt: String::new(),
            body,
            cover_url: None,
            published: true,
            created_at: String::new(),
        };
        assert_eq!(a.summary().chars().count(), 161);
    }
}
