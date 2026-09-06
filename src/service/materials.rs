//! service/materials.rs — "Materials Library" (migrasi 17): daftar + tambah
//! link + hapus. Upload FILE (audio/document/video) ditangani handler axum
//! murni `web/materials.rs` (butuh multipart, di luar server-fn) yang
//! memanggil `repository::materials` langsung setelah upload ke RustFS —
//! pola sama `web/live_audio.rs`.

use anyhow::Result;
use deadpool_postgres::Pool;

use super::fmt::wib;
use crate::models::MaterialItem;
use crate::repository as repo;

fn fmt_size(bytes: Option<i64>) -> String {
    match bytes {
        Some(b) if b >= 1_000_000 => format!("{:.1} MB", b as f64 / 1_000_000.0),
        Some(b) if b > 0 => format!("{:.1} KB", b as f64 / 1_000.0),
        _ => String::new(),
    }
}

fn kind_ext_label(kind: &str) -> &'static str {
    match kind {
        "audio" => "MP3",
        "document" => "PDF",
        "video" => "Video",
        _ => "Link",
    }
}

pub async fn list_materials(pool: &Pool, limit: i64) -> Result<Vec<MaterialItem>> {
    Ok(repo::recent_materials(pool, limit)
        .await?
        .into_iter()
        .map(|m| {
            let date = super::fmt::fmt_date(m.created_at.with_timezone(&wib()).date_naive());
            let size = fmt_size(m.file_size);
            let meta_label = if m.kind == "link" {
                format!("Link • {date}")
            } else if size.is_empty() {
                format!("{} • {date}", kind_ext_label(&m.kind))
            } else {
                format!("{} • {size} • {date}", kind_ext_label(&m.kind))
            };
            MaterialItem { id: m.id, title: m.title, kind: m.kind, file_url: m.file_url, meta_label }
        })
        .collect())
}

/// Panjang maksimum judul materi — mengikuti `materials.title VARCHAR(200)`.
///
/// Tanpa batas di sisi kode, judul yang lebih panjang menempuh SELURUH jalur
/// unggah lebih dulu — berkasnya sudah terkirim ke penyimpanan objek — lalu
/// gagal di `INSERT` dengan galat Postgres mentah ("value too long for type
/// character varying(200)") yang tak menyebut judul sama sekali. Yang dilihat
/// pengunggah cuma "gagal", dan berkasnya tertinggal yatim di bucket.
///
/// Angkanya menempel pada kolomnya: mengubah salah satunya tanpa yang lain
/// mengembalikan persis kegagalan itu.
pub const JUDUL_MAKS: usize = 200;

/// Periksa judul materi: tak kosong dan tak melebihi kolomnya.
///
/// Menghitung KARAKTER, bukan byte: Postgres `VARCHAR(200)` juga menghitung
/// karakter, dan judul berhuruf Arab memakai 2 byte per huruf — memakai
/// `len()` akan menolak judul yang sebenarnya muat.
pub fn periksa_judul(title: &str) -> Result<&str> {
    let title = title.trim();
    if title.is_empty() {
        bail_user!("Judul materi wajib diisi.");
    }
    if title.chars().count() > JUDUL_MAKS {
        bail_user!("Judul materi terlalu panjang (maks {JUDUL_MAKS} karakter).");
    }
    Ok(title)
}

/// Tambah materi berupa TAUTAN (mis. video YouTube) — tanpa upload file.
pub async fn add_link(pool: &Pool, uploaded_by: i64, title: &str, url: &str) -> Result<i64> {
    let title = periksa_judul(title)?;
    let url = url.trim();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        bail_user!("Tautan harus diawali http:// atau https://");
    }
    repo::insert_material(pool, None, uploaded_by, title, "link", url, None, None).await
}

/// Hapus materi; kembalikan URL berkasnya agar pemanggil membuangnya dari
/// penyimpanan objek. Kosong = tak ada berkas (materi berupa tautan).
pub async fn delete_material(pool: &Pool, id: i64) -> Result<String> {
    let (ada, url) = repo::delete_material(pool, id).await?;
    if !ada {
        bail_user!("Materi tidak ditemukan.");
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Judul wajar lolos apa adanya, sudah ter-trim.
    #[test]
    fn judul_wajar_diterima() {
        assert_eq!(periksa_judul("  Kitab Adab  ").unwrap(), "Kitab Adab");
    }

    #[test]
    fn judul_kosong_ditolak() {
        assert!(periksa_judul("").is_err());
        assert!(periksa_judul("   \n\t ").is_err());
    }

    /// WORST CASE — judul tepat di batas kolom `VARCHAR(200)` harus LOLOS.
    /// Batas yang meleset satu karakter menolak judul yang sebenarnya muat.
    #[test]
    fn judul_tepat_di_batas_diterima() {
        let tepat = "a".repeat(JUDUL_MAKS);
        assert!(periksa_judul(&tepat).is_ok());
    }

    /// WORST CASE — satu karakter di atas batas ditolak DI SINI, bukan nanti
    /// oleh Postgres sesudah berkasnya terlanjur terunggah.
    #[test]
    fn judul_lewat_batas_ditolak_lebih_awal() {
        let kepanjangan = "a".repeat(JUDUL_MAKS + 1);
        let e = periksa_judul(&kepanjangan).unwrap_err();
        assert!(
            e.to_string().contains("terlalu panjang"),
            "pesannya harus menyebut sebabnya: {e}"
        );
    }

    /// WORST CASE — judul berhuruf Arab.
    ///
    /// `VARCHAR(200)` menghitung KARAKTER, sedangkan tiap huruf Arab memakai 2
    /// byte. Memeriksa dengan `len()` akan menolak judul 150 huruf yang
    /// sebenarnya muat — dan menolaknya dengan alasan yang tak masuk akal bagi
    /// orang yang mengetiknya.
    #[test]
    fn judul_non_ascii_dihitung_per_karakter() {
        let arab = "ت".repeat(JUDUL_MAKS);
        assert!(arab.len() > JUDUL_MAKS, "prasyarat: byte > karakter");
        assert!(periksa_judul(&arab).is_ok(), "200 karakter harus muat");

        let kelebihan = "ت".repeat(JUDUL_MAKS + 1);
        assert!(periksa_judul(&kelebihan).is_err());
    }

    /// Spasi di ujung tak boleh ikut terhitung — judul 200 karakter yang
    /// tertempel bersama spasi tetap sah sesudah di-trim.
    #[test]
    fn spasi_ujung_tak_menghabiskan_jatah() {
        let dgn_spasi = format!("  {}  ", "a".repeat(JUDUL_MAKS));
        assert!(periksa_judul(&dgn_spasi).is_ok());
    }

    /// Batasnya HARUS sama dengan kolomnya. Kalau salah satunya diubah tanpa
    /// yang lain, kegagalan lamanya kembali — dan uji ini yang menahannya.
    #[test]
    fn batas_judul_sama_dengan_kolom_di_migrasi() {
        let m = std::fs::read_to_string("migration/17_new_structures.sql")
            .expect("migration/17_new_structures.sql hilang");
        assert!(
            m.contains(&format!("title        VARCHAR({JUDUL_MAKS}) NOT NULL")),
            "JUDUL_MAKS ({JUDUL_MAKS}) tak lagi cocok dengan kolom materials.title"
        );
    }
}
