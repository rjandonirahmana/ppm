//! service/sarana.rs — Validasi & penyusunan data sarana prasarana.

use anyhow::Result;
use deadpool_postgres::Pool;

use crate::models::{
    sarana_kategori_label, sarana_kondisi_label, SaranaData, SaranaItem, SaranaRingkas, SARANA_KATEGORI, SARANA_KONDISI,
};
use crate::repository as repo;

/// Panjang maksimum nama & lokasi — mengikuti `VARCHAR(120)` di migrasi 94.
///
/// Sama alasannya dengan `materials::JUDUL_MAKS`: tanpa batas di sisi kode,
/// masukan yang lebih panjang baru ditolak Postgres dengan galat constraint
/// mentah yang tak menyebut kolom mana yang bermasalah.
pub const NAMA_MAKS: usize = 120;

/// Batas jumlah unit per baris.
///
/// Bukan pagar keamanan melainkan pagar SALAH KETIK: "1200" yang dimaksud
/// "120" masih masuk akal dan harus lolos, sedangkan angka enam digit untuk
/// sebuah pondok hampir pasti jari yang tergelincir. Batas atas kolomnya
/// sendiri `INTEGER`, jauh lebih longgar dari yang berguna di sini.
pub const JUMLAH_MAKS: i32 = 100_000;

/// Berapa baris yang dimuat sekali baca.
///
/// Tanpa paginasi dengan sengaja: pendataan sarana sebuah pondok berjumlah
/// puluhan sampai ratusan baris, bukan puluhan ribu, dan seluruhnya justru
/// ingin terlihat sekaligus supaya bisa dicari dengan Ctrl-F. Batasnya tetap
/// ada sebagai pagar, bukan sebagai fitur.
const BATAS: i64 = 1_000;

fn sah(daftar: &[(&str, &str)], nilai: &str) -> bool {
    daftar.iter().any(|(v, _)| *v == nilai)
}

/// Periksa & rapikan masukan satu baris sarana.
///
/// Mengembalikan nilai yang sudah ter-trim supaya pemanggil tak bisa lupa
/// melakukannya — nama berspasi di ujung menghasilkan dua baris berbeda untuk
/// barang yang sama, dan itu persis yang membuat pendataan berhenti dipercaya.
///
/// Kategori & kondisi diperiksa terhadap daftar yang SAMA dengan yang dipakai
/// layar dan CHECK migrasi. Tanpa ini, nilai apa pun dari klien lolos sampai
/// database lalu ditolak sebagai galat constraint.
pub fn periksa(
    nama: &str,
    kategori: &str,
    lokasi: &str,
    jumlah: i32,
    kondisi: &str,
    catatan: &str,
) -> Result<(String, String, String, i32, String, String)> {
    let nama = nama.trim();
    if nama.is_empty() {
        bail_user!("Nama sarana wajib diisi.");
    }
    if nama.chars().count() > NAMA_MAKS {
        bail_user!("Nama sarana terlalu panjang (maks {NAMA_MAKS} karakter).");
    }
    let lokasi = lokasi.trim();
    if lokasi.chars().count() > NAMA_MAKS {
        bail_user!("Lokasi terlalu panjang (maks {NAMA_MAKS} karakter).");
    }
    if !sah(SARANA_KATEGORI, kategori) {
        bail_user!("Kategori tidak dikenal.");
    }
    if !sah(SARANA_KONDISI, kondisi) {
        bail_user!("Kondisi tidak dikenal.");
    }
    if jumlah <= 0 {
        bail_user!("Jumlah harus lebih dari 0.");
    }
    if jumlah > JUMLAH_MAKS {
        bail_user!("Jumlah terlalu besar (maks {JUMLAH_MAKS}).");
    }
    Ok((
        nama.to_string(),
        kategori.to_string(),
        lokasi.to_string(),
        jumlah,
        kondisi.to_string(),
        catatan.trim().to_string(),
    ))
}

/// Penyaring kosong = "semua". Nilai yang tak dikenal DIPERLAKUKAN SAMA, bukan
/// ditolak: penyaring adalah cara melihat, bukan perbuatan — menjawabnya dengan
/// galat karena satu parameter aneh di URL hanya membuat halaman mati tanpa
/// alasan yang berguna bagi pembacanya.
fn saring<'a>(daftar: &[(&str, &str)], v: &'a str) -> Option<&'a str> {
    (!v.is_empty() && sah(daftar, v)).then_some(v)
}

pub async fn list(pool: &Pool, kategori: &str, kondisi: &str) -> Result<SaranaData> {
    let (rows, ringkas) = tokio::try_join!(
        repo::list_sarana(pool, saring(SARANA_KATEGORI, kategori), saring(SARANA_KONDISI, kondisi), BATAS),
        repo::ringkas_sarana(pool),
    )?;

    Ok(SaranaData {
        items: rows
            .into_iter()
            .map(|r| SaranaItem {
                kategori_label: sarana_kategori_label(&r.kategori).into(),
                kondisi_label: sarana_kondisi_label(&r.kondisi).into(),
                diperbarui_label: super::fmt::tanggal_panjang(r.updated_at),
                id: r.id,
                nama: r.nama,
                kategori: r.kategori,
                lokasi: r.lokasi,
                jumlah: r.jumlah,
                kondisi: r.kondisi,
                catatan: r.catatan,
            })
            .collect(),
        // Ringkasan sengaja dihitung atas SELURUH tabel, bukan atas hasil yang
        // tersaring: ia menjawab "pondok ini punya apa", dan angka yang ikut
        // berubah tiap kali penyaring digeser tak bisa dipakai melapor.
        ringkas: SaranaRingkas {
            jenis: ringkas.0,
            unit: ringkas.1,
            perlu_perbaikan: ringkas.2,
        },
    })
}

#[allow(clippy::too_many_arguments)]
pub async fn simpan(
    pool: &Pool,
    id: Option<i64>,
    nama: &str,
    kategori: &str,
    lokasi: &str,
    jumlah: i32,
    kondisi: &str,
    catatan: &str,
    oleh: i64,
) -> Result<i64> {
    let (nama, kategori, lokasi, jumlah, kondisi, catatan) =
        periksa(nama, kategori, lokasi, jumlah, kondisi, catatan)?;

    match id {
        Some(id) => {
            let ada = repo::update_sarana(
                pool, id, &nama, &kategori, &lokasi, jumlah, &kondisi, &catatan, oleh,
            )
            .await?;
            if !ada {
                bail_user!("Data sarana tidak ditemukan — mungkin sudah dihapus orang lain.");
            }
            Ok(id)
        }
        None => {
            repo::insert_sarana(pool, &nama, &kategori, &lokasi, jumlah, &kondisi, &catatan, oleh)
                .await
        }
    }
}

pub async fn hapus(pool: &Pool, id: i64) -> Result<()> {
    if !repo::delete_sarana(pool, id).await? {
        bail_user!("Data sarana tidak ditemukan.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(nama: &str, jumlah: i32) -> Result<(String, String, String, i32, String, String)> {
        periksa(nama, "perabot", "Aula", jumlah, "baik", "")
    }

    #[test]
    fn masukan_wajar_diterima_dan_dirapikan() {
        let (n, k, l, j, kon, c) =
            periksa("  Kursi Santri  ", "perabot", "  Aula  ", 120, "baik", "  bekas  ").unwrap();
        assert_eq!((n.as_str(), l.as_str(), c.as_str()), ("Kursi Santri", "Aula", "bekas"));
        assert_eq!((k.as_str(), j, kon.as_str()), ("perabot", 120, "baik"));
    }

    #[test]
    fn nama_kosong_ditolak() {
        assert!(ok("", 1).is_err());
        assert!(ok("   ", 1).is_err());
    }

    /// Batas nama mengikuti kolomnya — tepat di batas harus LOLOS.
    #[test]
    fn nama_tepat_di_batas_diterima() {
        assert!(ok(&"a".repeat(NAMA_MAKS), 1).is_ok());
        assert!(ok(&"a".repeat(NAMA_MAKS + 1), 1).is_err());
    }

    /// Dihitung per KARAKTER — nama berhuruf Arab memakai 2 byte per huruf, dan
    /// memeriksa dengan `len()` menolak nama yang sebenarnya muat.
    #[test]
    fn nama_non_ascii_dihitung_per_karakter() {
        let arab = "ت".repeat(NAMA_MAKS);
        assert!(arab.len() > NAMA_MAKS);
        assert!(ok(&arab, 1).is_ok());
    }

    #[test]
    fn jumlah_harus_positif_dan_masuk_akal() {
        assert!(ok("Kursi", 0).is_err());
        assert!(ok("Kursi", -3).is_err());
        assert!(ok("Kursi", 1).is_ok());
        assert!(ok("Kursi", JUMLAH_MAKS).is_ok());
        assert!(ok("Kursi", JUMLAH_MAKS + 1).is_err());
        assert!(ok("Kursi", i32::MAX).is_err());
    }

    /// Nilai di luar daftar ditolak DI SINI, bukan dibiarkan sampai database
    /// lalu gagal sebagai galat constraint yang tak menyebut kolomnya.
    #[test]
    fn kategori_dan_kondisi_asing_ditolak() {
        assert!(periksa("Kursi", "entah", "Aula", 1, "baik", "").is_err());
        assert!(periksa("Kursi", "perabot", "Aula", 1, "hancur", "").is_err());
        assert!(periksa("Kursi", "", "Aula", 1, "baik", "").is_err());
    }

    /// Seluruh nilai yang ditawarkan layar harus lolos pemeriksaan — kalau
    /// tidak, dropdown-nya menjanjikan sesuatu yang gagal saat disimpan.
    #[test]
    fn setiap_pilihan_layar_lolos() {
        for (kat, _) in SARANA_KATEGORI {
            for (kon, _) in SARANA_KONDISI {
                assert!(periksa("Kursi", kat, "Aula", 1, kon, "").is_ok(), "{kat}/{kon}");
            }
        }
    }

    /// Lokasi boleh KOSONG — banyak barang memang tak punya letak tetap, dan
    /// memaksa mengisinya hanya menghasilkan tulisan "-" di seluruh kolom.
    #[test]
    fn lokasi_boleh_kosong() {
        assert!(periksa("Genset", "elektronik", "", 1, "baik", "").is_ok());
    }

    /// Penyaring: kosong dan nilai asing sama-sama berarti "semua", bukan galat.
    #[test]
    fn penyaring_kosong_dan_asing_berarti_semua() {
        assert_eq!(saring(SARANA_KATEGORI, ""), None);
        assert_eq!(saring(SARANA_KATEGORI, "entah"), None);
        assert_eq!(saring(SARANA_KATEGORI, "perabot"), Some("perabot"));
        assert_eq!(saring(SARANA_KONDISI, "rusak_berat"), Some("rusak_berat"));
    }

    /// Label & daftar sepadan dengan CHECK di migrasi — pilihan yang ada di
    /// layar tapi ditolak database adalah janji yang tak ditepati.
    #[test]
    fn daftar_sepadan_dengan_migrasi() {
        let m = std::fs::read_to_string("migration/94_sarana_prasarana.sql")
            .expect("migration/94 hilang");
        for (v, _) in SARANA_KATEGORI {
            assert!(m.contains(&format!("'{v}'")), "kategori {v} tak ada di CHECK");
        }
        for (v, _) in SARANA_KONDISI {
            assert!(m.contains(&format!("'{v}'")), "kondisi {v} tak ada di CHECK");
        }
    }

    #[test]
    fn label_tak_dikenal_punya_cadangan() {
        assert_eq!(sarana_kategori_label("perabot"), "Perabot");
        assert_eq!(sarana_kategori_label("entah"), "Lainnya");
        assert_eq!(sarana_kondisi_label("rusak_berat"), "Rusak Berat");
        assert_eq!(sarana_kondisi_label("entah"), "Baik");
    }
}
