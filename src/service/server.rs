//! service/server.rs — Pembacaan kesehatan mesin (CPU, memori, DISK, uptime)
//! untuk halaman /status-server.
//!
//! Sisa DISK satu-satunya yang tak bisa dibaca dari berkas semu — `/proc` cuma
//! memuat daftar mount, bukan kapasitasnya — jadi ia lewat `statvfs(2)` (lihat
//! [`ruang_disk`]). Sisanya dibaca dari berkas semu Linux (`/proc`,
//! `/sys/fs/cgroup`) yang memang disediakan kernel untuk keperluan ini. Sebuah
//! crate seperti `sysinfo` akan menambah puluhan detik waktu kompilasi dan
//! selusin dependensi transitif untuk empat berkas teks yang formatnya sudah
//! stabil sejak dua dekade.
//!
//! DI MAC/WINDOWS `/proc` tak ada, jadi seluruh pembacaan gagal dengan rapi dan
//! halaman menampilkan keterangan "tak tersedia di sistem operasi ini" alih-
//! alih deretan nol. Yang dipakai produksi adalah Linux (Docker di VPS).
//!
//! DI DALAM KONTAINER `/proc/meminfo` melaporkan memori MESIN INDUK, bukan
//! jatah kontainer. Karena itu batas cgroup v2 diperiksa lebih dulu: tanpa itu,
//! aplikasi yang dibatasi 1 GB tampak memakai 12% dari 8 GB — dan admin baru
//! tahu batasnya terlampaui ketika prosesnya sudah dibunuh OOM killer.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use deadpool_postgres::Pool;

use crate::models::{fmt_durasi, ServerStatus};

/// Saat proses ini mulai. Disentuh sekali di `main` supaya jamnya benar-benar
/// dimulai saat boot, bukan saat halaman status pertama kali dibuka.
static MULAI: LazyLock<Instant> = LazyLock::new(Instant::now);

/// Panggil sekali di awal `main()` — lihat [`MULAI`].
pub fn catat_waktu_mulai() {
    LazyLock::force(&MULAI);
}

/// Jeda antara dua cuplikan `/proc/stat`.
///
/// Pemakaian CPU adalah SELISIH dua pembacaan; satu pembacaan saja hanya
/// memberi total sejak boot, yang tak berarti apa-apa. 300 ms cukup untuk angka
/// yang stabil dan masih terasa seketika bagi yang menekan "Segarkan".
const JEDA_CUPLIK: Duration = Duration::from_millis(300);

fn baca(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok()
}

/// Angka pertama pada berkas cgroup satu-nilai (`memory.current`, `memory.max`).
/// `memory.max` berisi teks "max" saat tanpa batas — itu jadi `None`.
fn baca_angka(path: &str) -> Option<u64> {
    baca(path)?.trim().parse::<u64>().ok()
}

/// Jumlah jiffies (total, menganggur) dari baris pertama `/proc/stat`.
///
/// Kolomnya: user nice system idle iowait irq softirq steal …
/// "Menganggur" = idle + iowait; iowait ikut karena CPU-nya memang tak
/// mengerjakan apa pun saat itu, hanya menunggu disk.
fn cuplik_cpu() -> Option<(u64, u64)> {
    let isi = baca("/proc/stat")?;
    let baris = isi.lines().next()?;
    let angka: Vec<u64> =
        baris.split_whitespace().skip(1).filter_map(|v| v.parse::<u64>().ok()).collect();
    if angka.len() < 5 {
        return None;
    }
    let total: u64 = angka.iter().sum();
    let nganggur = angka[3] + angka[4];
    Some((total, nganggur))
}

/// Pemakaian CPU 0–100 persen seluruh mesin.
async fn cpu_pct() -> Option<f32> {
    let (t1, i1) = cuplik_cpu()?;
    tokio::time::sleep(JEDA_CUPLIK).await;
    let (t2, i2) = cuplik_cpu()?;
    let dt = t2.checked_sub(t1)?;
    let di = i2.saturating_sub(i1);
    if dt == 0 {
        return Some(0.0);
    }
    Some((((dt - di.min(dt)) as f64 / dt as f64) * 100.0) as f32)
}

/// Ambil satu nilai berlabel dari `/proc/meminfo`, dalam BYTE.
/// Format barisnya: `MemTotal:       16316576 kB`.
fn meminfo_kb(isi: &str, label: &str) -> Option<u64> {
    isi.lines()
        .find(|l| l.starts_with(label) && l[label.len()..].starts_with(':'))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse::<u64>().ok())
        .map(|kb| kb * 1024)
}

/// (total, terpakai, sumber). Batas kontainer menang atas memori mesin induk —
/// lihat catatan modul.
fn memori() -> Option<(u64, u64, String)> {
    // cgroup v2 (susunan yang dipakai Docker modern).
    let batas = baca_angka("/sys/fs/cgroup/memory.max");
    let pakai = baca_angka("/sys/fs/cgroup/memory.current");
    if let (Some(batas), Some(pakai)) = (batas, pakai) {
        // Sebagian cgroup dilaporkan dengan batas raksasa (praktis "tanpa
        // batas") — itu bukan angka yang berguna, jadi jatuh ke /proc/meminfo.
        if batas > 0 && batas < (1 << 60) {
            return Some((batas, pakai.min(batas), "Kontainer (cgroup)".into()));
        }
    }
    let isi = baca("/proc/meminfo")?;
    let total = meminfo_kb(&isi, "MemTotal")?;
    // MemAvailable, bukan MemFree: cache halaman bisa direbut kembali kapan
    // saja, jadi menghitungnya sebagai "terpakai" membuat setiap server Linux
    // yang sehat tampak nyaris kehabisan memori.
    let tersedia = meminfo_kb(&isi, "MemAvailable").unwrap_or_else(|| {
        meminfo_kb(&isi, "MemFree").unwrap_or(0)
            + meminfo_kb(&isi, "Cached").unwrap_or(0)
            + meminfo_kb(&isi, "Buffers").unwrap_or(0)
    });
    Some((total, total.saturating_sub(tersedia.min(total)), "Mesin (/proc/meminfo)".into()))
}

/// (total swap, swap terpakai) dalam byte.
fn swap() -> (u64, u64) {
    let Some(isi) = baca("/proc/meminfo") else { return (0, 0) };
    let total = meminfo_kb(&isi, "SwapTotal").unwrap_or(0);
    let bebas = meminfo_kb(&isi, "SwapFree").unwrap_or(0);
    (total, total.saturating_sub(bebas.min(total)))
}

/// Memori yang dipakai proses aplikasi ini sendiri (RSS), dalam byte.
fn app_rss() -> u64 {
    baca("/proc/self/status")
        .and_then(|isi| meminfo_kb(&isi, "VmRSS"))
        .unwrap_or(0)
}

// ── Penyimpanan (SSD/NVMe) ───────────────────────────────────────────────────

/// (total, terpakai, tersedia) satu filesystem, dalam byte.
///
/// `statvfs(2)`, bukan `/proc`: berkas semu itu memuat daftar mount, bukan
/// kapasitasnya — ruang kosong memang hanya tersedia lewat syscall ini. Juga
/// bukan memanggil `df`: itu berarti menjalankan proses tiap halaman dibuka dan
/// bergantung pada base image yang menyertakan coreutils.
///
/// Berjalan di macOS juga (statvfs ada di POSIX), jadi kartu penyimpanan tetap
/// berisi di mesin pengembang meski CPU/memori-nya tidak.
fn ruang_disk(path: &std::path::Path) -> Option<(u64, u64, u64)> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: `c` adalah C-string valid berumur panjang selama pemanggilan, dan
    // `s` adalah statvfs milik kita yang di-nol-kan lebih dulu. statvfs hanya
    // MENULIS ke `s` dan tak menyimpan pointer apa pun.
    let s = unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) != 0 {
            return None;
        }
        s
    };
    // f_frsize = ukuran blok sesungguhnya; f_bsize hanya ukuran blok "yang
    // disukai" I/O dan pada sebagian filesystem berbeda dari yang dipakai
    // menghitung f_blocks. Salah memilih = angka melenceng berkali lipat.
    let unit = if s.f_frsize > 0 { s.f_frsize as u64 } else { s.f_bsize as u64 };
    let total = (s.f_blocks as u64).checked_mul(unit)?;
    let bebas_root = (s.f_bfree as u64).saturating_mul(unit);
    let tersedia = (s.f_bavail as u64).saturating_mul(unit);
    Some((total, total.saturating_sub(bebas_root), tersedia))
}

/// Jalur yang benar-benar ADA terdekat dari `path`, menaiki induknya.
///
/// `RECORDINGS_DIR` & `UPLOAD_TMP_DIR` dibuat saat pertama dipakai, jadi di
/// server yang belum pernah merekam direktorinya belum ada — dan statvfs atas
/// jalur yang tak ada gagal. Yang ingin diketahui admin toh disk TEMPAT
/// direktori itu akan lahir, dan induknya ada di disk yang sama.
fn jalur_terdekat(path: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut p = path.to_path_buf();
    loop {
        if p.exists() {
            return Some(p);
        }
        if !p.pop() || p.as_os_str().is_empty() {
            return None;
        }
    }
}

/// Ruang disk untuk tiap FILESYSTEM yang dipakai aplikasi.
///
/// Tiga jalur diperiksa — disk sistem, direktori rekaman, direktori berkas
/// sementara unggahan — lalu yang berada di filesystem yang SAMA dibuang
/// (dibandingkan lewat nomor device `st_dev`). Di penataan biasa hasilnya satu
/// kartu; entri kedua muncul justru saat rekaman ditaruh di volume terpisah,
/// dan itu persis keadaan yang perlu dilihat sendiri-sendiri.
fn daftar_disk() -> Vec<crate::models::DiskInfo> {
    use std::os::unix::fs::MetadataExt;

    let kandidat: [(&str, std::path::PathBuf); 3] = [
        ("Disk sistem", std::path::PathBuf::from("/")),
        ("Rekaman sesi", crate::web::live_audio::recordings_dir()),
        ("Unggahan sementara", crate::web::multipart::upload_tmp_dir()),
    ];

    let mut hasil = Vec::new();
    let mut device_terlihat: Vec<u64> = Vec::new();
    for (label, path) in kandidat {
        let Some(nyata) = jalur_terdekat(&path) else { continue };
        let Ok(dev) = std::fs::metadata(&nyata).map(|m| m.dev()) else { continue };
        if device_terlihat.contains(&dev) {
            continue;
        }
        let Some((total, terpakai, tersedia)) = ruang_disk(&nyata) else { continue };
        if total == 0 {
            continue;
        }
        device_terlihat.push(dev);
        hasil.push(crate::models::DiskInfo {
            label: label.into(),
            path: path.display().to_string(),
            total,
            terpakai,
            tersedia,
            pct: crate::models::pct_disk(terpakai, tersedia),
        });
    }
    hasil
}

fn loadavg() -> (f32, f32, f32) {
    let Some(isi) = baca("/proc/loadavg") else { return (0.0, 0.0, 0.0) };
    let mut n = isi.split_whitespace().filter_map(|v| v.parse::<f32>().ok());
    (n.next().unwrap_or(0.0), n.next().unwrap_or(0.0), n.next().unwrap_or(0.0))
}

fn uptime_mesin_detik() -> u64 {
    baca("/proc/uptime")
        .and_then(|isi| isi.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()))
        .map(|d| d as u64)
        .unwrap_or(0)
}

/// Potret keadaan server sekarang. Butuh `pool` untuk status kolam koneksi —
/// satu-satunya angka di halaman ini yang tak datang dari sistem operasi, dan
/// justru yang paling sering menjelaskan "kenapa aplikasinya lambat padahal
/// CPU-nya santai".
pub async fn status(pool: &Pool) -> ServerStatus {
    let st = pool.status();
    let uptime_app = fmt_durasi(MULAI.elapsed().as_secs());
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);

    // Semua pembacaan Linux dianggap satu paket: bila memori tak terbaca,
    // sisanya pun tidak, dan menampilkan sebagian angka lebih menyesatkan
    // daripada mengatakan terus terang bahwa sumbernya tak ada.
    let Some((mem_total, mem_terpakai, mem_sumber)) = memori() else {
        return ServerStatus {
            tersedia: false,
            catatan: format!(
                "Angka CPU & memori dibaca dari /proc — tersedia di server Linux \
                 (produksi), tidak di {}. Penyimpanan & kolam database di bawah tetap nyata.",
                std::env::consts::OS
            ),
            cpu_pct: 0.0,
            cpu_cores: cores,
            load1: 0.0,
            load5: 0.0,
            load15: 0.0,
            mem_total: 0,
            mem_terpakai: 0,
            mem_pct: 0.0,
            mem_sumber: "—".into(),
            swap_total: 0,
            swap_terpakai: 0,
            app_rss: 0,
            // Disk TIDAK ikut dikosongkan: `statvfs` POSIX, jadi angkanya benar
            // di macOS juga. Menolaknya di sini cuma menyembunyikan data yang
            // sudah ada di tangan.
            disk: daftar_disk(),
            uptime_mesin: "—".into(),
            uptime_app,
            pool_max: st.max_size,
            pool_size: st.size,
            pool_idle: st.available.max(0) as usize,
        };
    };

    let (swap_total, swap_terpakai) = swap();
    let (load1, load5, load15) = loadavg();
    let mem_pct =
        if mem_total > 0 { (mem_terpakai as f64 / mem_total as f64 * 100.0) as f32 } else { 0.0 };

    ServerStatus {
        tersedia: true,
        catatan: format!(
            "CPU dicuplik dua kali berjarak {} ms; memori dari {mem_sumber}.",
            JEDA_CUPLIK.as_millis()
        ),
        cpu_pct: cpu_pct().await.unwrap_or(0.0),
        cpu_cores: cores,
        load1,
        load5,
        load15,
        mem_total,
        mem_terpakai,
        mem_pct,
        mem_sumber,
        swap_total,
        swap_terpakai,
        app_rss: app_rss(),
        disk: daftar_disk(),
        uptime_mesin: fmt_durasi(uptime_mesin_detik()),
        uptime_app,
        pool_max: st.max_size,
        pool_size: st.size,
        pool_idle: st.available.max(0) as usize,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTOH: &str = "MemTotal:       16316576 kB\n\
                          MemFree:          123456 kB\n\
                          MemAvailable:    8158288 kB\n\
                          Buffers:           10240 kB\n\
                          SwapTotal:       2097152 kB\n\
                          SwapFree:        2097152 kB\n";

    #[test]
    fn membaca_label_meminfo_dalam_byte() {
        assert_eq!(meminfo_kb(CONTOH, "MemTotal"), Some(16_316_576 * 1024));
        assert_eq!(meminfo_kb(CONTOH, "MemAvailable"), Some(8_158_288 * 1024));
        assert_eq!(meminfo_kb(CONTOH, "SwapFree"), Some(2_097_152 * 1024));
        assert_eq!(meminfo_kb(CONTOH, "TidakAda"), None);
    }

    /// `statvfs` atas root harus selalu berhasil di Linux MAUPUN macOS, dan
    /// angkanya harus masuk akal. Uji ini yang menangkap salah pilih satuan
    /// blok (f_bsize vs f_frsize) — gejalanya total meleset berkali lipat, yang
    /// tak kentara kalau hanya dilihat sekilas di layar.
    #[test]
    fn ruang_disk_root_terbaca_dan_konsisten() {
        let (total, terpakai, tersedia) =
            ruang_disk(std::path::Path::new("/")).expect("statvfs / harus berhasil");
        assert!(total > 0);
        assert!(terpakai <= total);
        assert!(tersedia <= total);
        // Cadangan root membuat tersedia ≤ sisa kasar, tak pernah lebih.
        assert!(tersedia <= total - terpakai);
    }

    /// Direktori yang belum ada harus jatuh ke induknya, bukan hilang dari
    /// daftar — RECORDINGS_DIR baru lahir saat siaran pertama.
    #[test]
    fn jalur_belum_ada_naik_ke_induk() {
        let p = std::path::Path::new("/tmp/ppm-tak-ada-xyz/lagi/dalam");
        let hasil = jalur_terdekat(p).expect("harus menemukan induk yang ada");
        assert!(hasil.exists());
    }

    /// "MemFree" TIDAK boleh cocok saat yang dicari "Mem": tanpa pemeriksaan
    /// titik dua, `starts_with` mencocokkan label mana pun yang berawalan sama
    /// dan angka yang terbaca jadi milik baris yang salah.
    #[test]
    fn label_dicocokkan_utuh_bukan_awalan() {
        assert_eq!(meminfo_kb(CONTOH, "Mem"), None);
        assert_eq!(meminfo_kb(CONTOH, "Swap"), None);
        assert_eq!(meminfo_kb(CONTOH, "MemFree"), Some(123_456 * 1024));
    }
}
