//! models/hafalan.rs — Kerangka laporan akademik kategori "Mengaji"/"Pengajian":
//! tipe tampilan setoran hafalan + matcher kategori kelas.

use serde::{Deserialize, Serialize};

/// Kelas kategori "mengaji" (hafalan/tahfidz/pengajian Qur'an) — dipakai
/// mengelompokkan laporan akademik & memunculkan panel Setoran Hafalan di
/// detail sesi. LEBIH LONGGAR dari `category_allows_recording` (gerbang suara
/// hanya "Pengajian" persis) — di sini cukup MENGANDUNG kata "mengaji" atau
/// "pengajian", krn kategori kelas teks bebas (staf boleh ketik variasi).
pub fn is_mengaji_category(category: &str) -> bool {
    let c = category.trim().to_lowercase();
    c.contains("mengaji") || c.contains("pengajian") || c.contains("tahfidz") || c.contains("hafalan")
}

/// Satu baris setoran hafalan (riwayat santri / staf pencatat).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HafalanItem {
    pub id: i64,
    pub surah: String,
    /// "1–40" atau kosong bila tak dicatat.
    pub ayat_range: String,
    pub juz: Option<i16>,
    /// lancar | perlu_perbaikan | mengulang
    pub quality: String,
    /// "Lancar" / "Perlu Perbaikan" / "Mengulang"
    pub quality_label: String,
    pub note: String,
    /// "20 Jul 2026"
    pub date_label: String,
    /// Nama pencatat (staf), "-" bila tak diketahui.
    pub recorded_by: String,
}

/// Satu baris "Santri Teladan" (ranking hafalan+poin) — dipakai laporan kelas
/// akademik dewan guru.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HafalanRankItem {
    pub user_id: i64,
    pub name: String,
    pub class_name: String,
    pub juz_count: i64,
    pub points: i64,
}

pub fn quality_label(q: &str) -> &'static str {
    match q {
        "perlu_perbaikan" => "Perlu Perbaikan",
        "mengulang" => "Mengulang",
        _ => "Lancar",
    }
}

#[cfg(test)]
mod tests {
    use super::is_mengaji_category;

    #[test]
    fn kategori_mengaji_dikenali() {
        assert!(is_mengaji_category("Pengajian"));
        assert!(is_mengaji_category("Mengaji Subuh"));
        assert!(is_mengaji_category("  TAHFIDZ  "));
        assert!(is_mengaji_category("Hafalan Qur'an"));
        assert!(!is_mengaji_category("Sholat"));
        assert!(!is_mengaji_category("Cepatan"));
        assert!(!is_mengaji_category(""));
    }
}
