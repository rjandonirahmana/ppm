//! models/sarana.rs — Sarana & prasarana pondok.
//!
//! Skema & alasan bentuknya ada di `migration/94_sarana_prasarana.sql`.

use serde::{Deserialize, Serialize};

/// Kategori sarana. Nilainya SEPADAN PERSIS dengan CHECK di migrasi 94 —
/// menyimpang berarti pilihan yang ada di layar ditolak database saat disimpan.
///
/// Pasangan (nilai, label): nilai untuk disimpan, label untuk dibaca orang.
pub const SARANA_KATEGORI: &[(&str, &str)] = &[
    ("gedung", "Gedung"),
    ("ruang", "Ruang"),
    ("perabot", "Perabot"),
    ("elektronik", "Elektronik"),
    ("kendaraan", "Kendaraan"),
    ("ibadah", "Perlengkapan Ibadah"),
    ("olahraga", "Olahraga"),
    ("lainnya", "Lainnya"),
];

/// Keadaan barang. Tiga tingkat, bukan lebih: yang menentukan tindakan hanyalah
/// "dipakai apa adanya", "perlu diperbaiki", dan "tak bisa dipakai". Tingkat
/// keempat hanya membuat dua orang menilai barang yang sama secara berbeda.
pub const SARANA_KONDISI: &[(&str, &str)] = &[
    ("baik", "Baik"),
    ("rusak_ringan", "Rusak Ringan"),
    ("rusak_berat", "Rusak Berat"),
];

fn label_dari(daftar: &[(&str, &'static str)], nilai: &str) -> &'static str {
    daftar
        .iter()
        .find(|(v, _)| *v == nilai)
        .map(|(_, l)| *l)
        .unwrap_or("Lainnya")
}

pub fn sarana_kategori_label(v: &str) -> &'static str {
    label_dari(SARANA_KATEGORI, v)
}

/// Label kondisi. Nilai tak dikenal jatuh ke "Baik" — BUKAN "Lainnya", yang
/// tak berarti apa-apa untuk sebuah keadaan barang.
pub fn sarana_kondisi_label(v: &str) -> &'static str {
    SARANA_KONDISI
        .iter()
        .find(|(k, _)| *k == v)
        .map(|(_, l)| *l)
        .unwrap_or("Baik")
}

/// Kelas lencana kondisi — mengikuti pola yang sudah dipakai seluruh aplikasi
/// (`ppm-chip` + utilitas warna Tailwind di tempat, lihat `pages/izin_aktif.rs`)
/// alih-alih memperkenalkan kelas baru yang hanya dipakai satu halaman.
///
/// Rusak berat memakai warna GALAT, bukan sekadar peringatan: barang yang tak
/// bisa dipakai menuntut tindakan, dan daftar yang mewarnainya sama dengan
/// "rusak ringan" membuat keduanya sama-sama mudah dilewati mata.
pub fn sarana_kondisi_warna(v: &str) -> &'static str {
    match v {
        "rusak_berat" => "ppm-chip bg-error/10 text-error shrink-0",
        "rusak_ringan" => "ppm-chip bg-warning/15 text-warning shrink-0",
        _ => "ppm-chip bg-success/10 text-success shrink-0",
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaranaItem {
    pub id: i64,
    pub nama: String,
    pub kategori: String,
    pub kategori_label: String,
    pub lokasi: String,
    pub jumlah: i32,
    pub kondisi: String,
    pub kondisi_label: String,
    pub catatan: String,
    /// "3 Sep 2026" — sudah diformat di service, seperti seluruh layar lain.
    pub diperbarui_label: String,
}

/// Ringkasan untuk kepala halaman.
///
/// Tiga angka yang benar-benar ditanyakan pengurus: berapa jenis barang yang
/// terdata, berapa unit seluruhnya, dan berapa yang butuh perbaikan. Yang
/// terakhir itu satu-satunya yang menuntut tindakan, jadi ia berdiri sendiri.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SaranaRingkas {
    pub jenis: i64,
    pub unit: i64,
    pub perlu_perbaikan: i64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct SaranaData {
    pub items: Vec<SaranaItem>,
    pub ringkas: SaranaRingkas,
}
