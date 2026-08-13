//! models/server.rs — Kesehatan mesin tempat aplikasi berjalan (halaman
//! /status-server, admin). Dikompilasi untuk SSR **dan** WASM, jadi isinya
//! angka-angka jadi — bukan pembacaan `/proc` (itu di `service/server.rs`).
//!
//! Semua ukuran memori dalam BYTE. Pemformatan ke "1,4 GB" dilakukan di sini
//! (`fmt_bytes`) supaya server dan layar mustahil berbeda pembulatan.

use serde::{Deserialize, Serialize};

/// Satu potret keadaan server saat halaman dibuka.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServerStatus {
    /// Angka mesin (CPU/memori) berhasil dibaca?
    ///
    /// `false` di macOS/Windows: pembacaannya lewat `/proc`, yang hanya ada di
    /// Linux. Halaman tetap tampil — dengan `catatan` yang menjelaskan — alih-
    /// alih memajang nol yang terbaca seperti server sedang menganggur total.
    pub tersedia: bool,
    /// Penjelasan singkat asal angkanya, atau alasan tak tersedia.
    pub catatan: String,

    // ── CPU ──────────────────────────────────────────────────────────────
    /// Pemakaian CPU 0–100 persen, hasil dua cuplikan `/proc/stat`.
    pub cpu_pct: f32,
    /// Jumlah inti yang terlihat oleh proses ini.
    pub cpu_cores: usize,
    /// Rata-rata beban 1/5/15 menit. Nilainya BUKAN persen: 2,0 pada mesin
    /// 2 inti berarti antrean tepat penuh, pada 4 inti berarti setengah.
    pub load1: f32,
    pub load5: f32,
    pub load15: f32,

    // ── Memori ───────────────────────────────────────────────────────────
    pub mem_total: u64,
    pub mem_terpakai: u64,
    pub mem_pct: f32,
    /// "Kontainer (cgroup)" atau "Mesin (/proc/meminfo)" — penting karena
    /// keduanya bisa berbeda jauh, dan angka tanpa keterangan asalnya membuat
    /// admin menyimpulkan yang salah tentang sisa memorinya.
    pub mem_sumber: String,
    pub swap_total: u64,
    pub swap_terpakai: u64,
    /// Memori yang dipakai proses aplikasi ini sendiri (RSS).
    pub app_rss: u64,

    // ── Penyimpanan (SSD/NVMe) ───────────────────────────────────────────
    /// Satu entri per FILESYSTEM yang dipakai aplikasi (biasanya satu: disk
    /// sistem). Kosong hanya bila pembacaannya gagal total.
    ///
    /// `#[serde(default)]`: payload lama (sebelum kartu penyimpanan ada) tak
    /// punya medan ini, dan tanpa default satu respons basi dari cache akan
    /// menggagalkan SELURUH halaman alih-alih kehilangan satu kartu.
    #[serde(default)]
    pub disk: Vec<DiskInfo>,

    // ── Waktu hidup ──────────────────────────────────────────────────────
    /// "3 hari 4 jam" — sejak mesin menyala.
    pub uptime_mesin: String,
    /// "5 jam 12 menit" — sejak proses aplikasi mulai. Selisih besar dengan
    /// uptime mesin berarti aplikasi pernah restart tanpa mesinnya ikut.
    pub uptime_app: String,

    // ── Kolam koneksi database ───────────────────────────────────────────
    /// Batas koneksi (DB_POOL_MAX_SIZE).
    pub pool_max: usize,
    /// Koneksi yang sudah terbentuk.
    pub pool_size: usize,
    /// Koneksi menganggur & siap dipakai. Nol terus-menerus = permintaan
    /// sedang antre menunggu koneksi, gejala paling awal dari halaman lambat.
    pub pool_idle: usize,
}

/// Ruang satu filesystem (SSD/NVMe VPS) — kartu "Penyimpanan" /status-server.
///
/// Yang paling penting di halaman ini adalah [`tersedia`](Self::tersedia).
/// Memori penuh membuat aplikasi dibunuh lalu dihidupkan lagi oleh Docker;
/// DISK penuh membuat Postgres berhenti menerima tulisan, unggahan gagal
/// separuh jalan, dan rekaman sesi terpotong — dan tak satu pun dari itu pulih
/// sendiri setelah restart. Karena itu angka besar di kartunya adalah SISA,
/// bukan yang terpakai.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DiskInfo {
    /// "Disk sistem", "Rekaman sesi", "Unggahan sementara".
    pub label: String,
    /// Jalur yang diperiksa (apa adanya, termasuk bila relatif seperti
    /// `./recordings`) — supaya admin tahu env mana yang mengarahkannya.
    pub path: String,
    pub total: u64,
    pub terpakai: u64,
    /// Ruang yang benar-benar bisa dipakai proses biasa. BUKAN `total −
    /// terpakai`: ext4 menyimpan jatah cadangan untuk root (bawaan 5%), jadi
    /// dua angka itu memang berbeda dan yang menentukan kapan tulisan mulai
    /// gagal adalah yang ini.
    pub tersedia: u64,
    /// Persen terpakai, dihitung sama dengan `df`: terpakai / (terpakai +
    /// tersedia) — bukan terhadap total. Kalau tidak, disk yang `df` sebut 100%
    /// akan tampil 95% di sini dan angkanya jadi tak bisa dibandingkan.
    pub pct: f32,
}

/// (terpakai, tersedia) → persen terpakai ala `df`. Dipisah supaya bisa diuji
/// tanpa menyentuh filesystem.
pub fn pct_disk(terpakai: u64, tersedia: u64) -> f32 {
    let dasar = terpakai.saturating_add(tersedia);
    if dasar == 0 {
        return 0.0;
    }
    (terpakai as f64 / dasar as f64 * 100.0) as f32
}

/// Peringatan sisa disk, atau None bila masih lapang.
///
/// DUA syarat, bukan satu persen saja: 10% dari 500 GB masih 50 GB (lapang),
/// sementara 10% dari 20 GB tinggal 2 GB (satu malam rekaman sesi). Ambang
/// mutlaknya yang menangkap VPS kecil — dan VPS kecil justru yang dipakai.
pub fn peringatan_disk(tersedia: u64, pct: f32) -> Option<&'static str> {
    const GB: u64 = 1024 * 1024 * 1024;
    if tersedia < 2 * GB || pct >= 95.0 {
        Some(
            "Sisa disk KRITIS. Postgres berhenti menerima tulisan bila disk penuh, dan \
             unggahan/rekaman akan gagal separuh jalan. Kosongkan sekarang: rekaman lama \
             di RECORDINGS_DIR, berkas sisa di UPLOAD_TMP_DIR, lalu `docker system prune`.",
        )
    } else if tersedia < 5 * GB || pct >= 85.0 {
        Some(
            "Sisa disk menipis. Periksa rekaman sesi lama (yang sudah terunggah ke RustFS \
             boleh dihapus), berkas sementara unggahan, dan image Docker yang tak terpakai.",
        )
    } else {
        None
    }
}

/// Ukuran byte → "812 MB" / "3,7 GB". Basis 1024 (yang dipakai `free`, htop,
/// dan cgroup), bukan 1000 — supaya angkanya cocok saat admin membandingkan
/// dengan perintah di terminal.
pub fn fmt_bytes(b: u64) -> String {
    const KB: f64 = 1024.0;
    let b = b as f64;
    let (nilai, satuan) = if b >= KB * KB * KB {
        (b / (KB * KB * KB), "GB")
    } else if b >= KB * KB {
        (b / (KB * KB), "MB")
    } else if b >= KB {
        (b / KB, "KB")
    } else {
        (b, "B")
    };
    // Satu desimal untuk GB (3,7 GB bermakna), bulat untuk sisanya (812,3 MB
    // tak menambah apa pun bagi pembaca).
    if satuan == "GB" {
        format!("{:.1} {}", nilai, satuan).replace('.', ",")
    } else {
        format!("{:.0} {}", nilai, satuan)
    }
}

/// Detik → "3 hari 4 jam" / "12 menit". Dipakai untuk uptime.
pub fn fmt_durasi(detik: u64) -> String {
    let hari = detik / 86_400;
    let jam = (detik % 86_400) / 3_600;
    let menit = (detik % 3_600) / 60;
    if hari > 0 {
        format!("{hari} hari {jam} jam")
    } else if jam > 0 {
        format!("{jam} jam {menit} menit")
    } else if menit > 0 {
        format!("{menit} menit")
    } else {
        format!("{detik} detik")
    }
}

/// Warna/keparahan sebuah persentase pemakaian — SATU ambang untuk CPU maupun
/// memori supaya dua kartu bersebelahan tak memakai skala yang berbeda diam-diam.
///
/// Mengembalikan (label, kelas warna Tailwind).
pub fn tingkat_pakai(pct: f32) -> (&'static str, &'static str) {
    if pct >= 90.0 {
        ("Kritis", "text-error")
    } else if pct >= 75.0 {
        ("Tinggi", "text-warning")
    } else if pct >= 50.0 {
        ("Sedang", "text-on-background")
    } else {
        ("Aman", "text-success")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ukuran_byte_terbaca_manusia() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(2 * 1024), "2 KB");
        assert_eq!(fmt_bytes(812 * 1024 * 1024), "812 MB");
        assert_eq!(fmt_bytes(4 * 1024 * 1024 * 1024), "4,0 GB");
    }

    /// Batas GB memakai basis 1024: 1.000.000.000 byte BUKAN 1 GB di sini,
    /// supaya angkanya sama dengan yang dilihat admin di `free -h`.
    #[test]
    fn basis_1024_bukan_1000() {
        assert_eq!(fmt_bytes(1_000_000_000), "954 MB");
        assert_eq!(fmt_bytes(1_073_741_824), "1,0 GB");
    }

    #[test]
    fn durasi_memilih_satuan_terbesar() {
        assert_eq!(fmt_durasi(45), "45 detik");
        assert_eq!(fmt_durasi(300), "5 menit");
        assert_eq!(fmt_durasi(7_320), "2 jam 2 menit");
        assert_eq!(fmt_durasi(273_600), "3 hari 4 jam");
    }

    /// Persen disk dihitung ala `df` (terhadap terpakai+tersedia), BUKAN
    /// terhadap total — kalau tidak, angkanya tak cocok dengan `df -h` di
    /// terminal karena jatah cadangan root ikut terhitung sebagai "sisa".
    #[test]
    fn pct_disk_mengikuti_df() {
        // 9 GB terpakai, 1 GB tersedia → 90%, meski totalnya 10,5 GB.
        let gb = 1024 * 1024 * 1024;
        assert_eq!(pct_disk(9 * gb, gb).round(), 90.0);
        assert_eq!(pct_disk(0, 0), 0.0);
        assert_eq!(pct_disk(gb, 0).round(), 100.0);
    }

    #[test]
    fn peringatan_disk_bertingkat() {
        let gb = 1024 * 1024 * 1024;
        assert!(peringatan_disk(40 * gb, 20.0).is_none());
        // Persen kecil tapi sisa mutlaknya sedikit — VPS kecil harus tertangkap.
        assert!(peringatan_disk(3 * gb, 10.0).is_some());
        assert!(peringatan_disk(gb, 10.0).unwrap().contains("KRITIS"));
        // Sisa besar tapi persennya ekstrem — disk besar yang hampir penuh.
        assert!(peringatan_disk(50 * gb, 96.0).unwrap().contains("KRITIS"));
    }

    /// Ambangnya harus MENAIK — kalau tidak, memori 95% bisa tampil "Aman".
    #[test]
    fn tingkat_pakai_naik_bertahap() {
        assert_eq!(tingkat_pakai(10.0).0, "Aman");
        assert_eq!(tingkat_pakai(60.0).0, "Sedang");
        assert_eq!(tingkat_pakai(80.0).0, "Tinggi");
        assert_eq!(tingkat_pakai(99.0).0, "Kritis");
    }
}
