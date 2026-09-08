//! tests/tanpa_query_dalam_loop.rs — TIDAK BOLEH ADA query di dalam loop.
//!
//! ── KENAPA INI ATURAN, BUKAN SEKADAR SARAN ───────────────────────────────────
//! Query di dalam loop (“N+1”) tak pernah terlihat salah saat ditulis: satu
//! baris yang memanggil satu fungsi yang sudah ada. Biayanya baru muncul dari
//! JUMLAH putarannya, dan jumlah itu ditentukan DATA — jadi ia lolos setiap
//! pengujian di laptop, lalu tumbuh diam-diam bersama isi database.
//!
//! Yang sudah pernah ada di pohon ini sebelum aturan ini ditegakkan:
//!
//!   • `service::rekap::credit_weekly_rewards` — per santri: ambil koneksi
//!     pool, BEGIN, dua INSERT, COMMIT. Untuk 300 santri itu ±1.500 perjalanan
//!     bolak-balik dan 300 transaksi, berurutan, sambil menahan pool berisi 16.
//!     Satu tombol admin cukup untuk membuat seluruh aplikasi tersendat.
//!   • `service::kelas::kelas_saya` — TIGA query per kelas, di jalur request.
//!     Seorang wali dengan delapan kelas membayar 24 perjalanan tiap kali
//!     membuka halamannya, dan jumlah kelas seseorang tak dibatasi apa pun.
//!
//! Keduanya kini set-based. Tes ini yang menjaga agar tak ada yang ketiga.
//!
//! ── APA YANG DIANGGAP “QUERY” ────────────────────────────────────────────────
//! Pemanggilan LANGSUNG ke lapisan database: `repo::…()`, `.query…()`,
//! `.execute()`, `.prepare()`, `pool.get()`. Fungsi `service::…` yang di
//! dalamnya menyentuh DB TIDAK terdeteksi — pemeriksa teks tak bisa
//! menelusurinya. Jadi lulusnya tes ini bukan bukti tak ada N+1 sama sekali;
//! ia menangkap bentuk yang paling sering ditulis, dan itu sudah cukup untuk
//! membuat orang berhenti sejenak sebelum menulisnya lagi.
//!
//! ── DUA BENTUK YANG DIKECUALIKAN ─────────────────────────────────────────────
//! 1. `loop { tick.tick().await; … }` — penjadwal tugas latar di `main.rs`. Ia
//!    memang berulang selamanya, tapi sekali tiap 24 jam, dan “putaran”-nya
//!    bukan baris data melainkan waktu. Dikenali dari `.tick().await` di
//!    badannya, bukan dari nama berkasnya, supaya pengecualiannya menjelaskan
//!    dirinya sendiri.
//! 2. Loop COBA-LAGI-SAAT-BENTROK, didaftar satu per satu di [`DIKECUALIKAN`]
//!    beserta alasannya. Bedanya dengan N+1 bukan soal derajat melainkan soal
//!    apa yang menggerakkan putarannya: N+1 berputar sebanyak BARIS DATA,
//!    sedangkan loop coba-lagi berputar hanya saat sebuah percobaan BENAR-BENAR
//!    ditolak — normalnya sekali, dan batas atasnya konstanta di kode.

use std::fs;
use std::path::{Path, PathBuf};

/// Fungsi yang loop-nya BOLEH memuat query, beserta alasannya.
///
/// Sengaja per-FUNGSI, bukan per-berkas: mengecualikan seluruh berkas berarti
/// N+1 berikutnya yang ditulis di sana ikut lolos diam-diam. Dan sengaja
/// membawa `alasan` yang ikut tercetak — menambah baris di sini harus terasa
/// seperti mengambil keputusan, bukan seperti mematikan alarm.
const DIKECUALIKAN: &[(&str, &str, &str)] = &[(
    "src/repository/articles.rs",
    "insert_article",
    "Loop COBA-LAGI saat slug bentrok, bukan iterasi atas data: normalnya \
     berjalan SATU query, dan putaran tambahan hanya terjadi bila INSERT \
     benar-benar ditolak indeks unik. Batasnya konstanta (MAKS_PERCOBAAN_SLUG). \
     Menyatukannya jadi satu pernyataan (`generate_series` + `NOT EXISTS`) \
     justru mengembalikan balapan yang doc fungsi itu jelaskan sudah sengaja \
     dihindari — celah antara memeriksa dan menyisipkan — sehingga retry-nya \
     tetap dibutuhkan dan tak ada yang dihemat.",
)];

/// Nama fungsi terakhir yang dibuka sebelum baris `n` (0-based).
fn fungsi_di_baris(baris: &[&str], n: usize) -> String {
    baris[..=n]
        .iter()
        .rev()
        .find_map(|l| {
            let t = l.trim_start();
            let sisa = t
                .strip_prefix("pub async fn ")
                .or_else(|| t.strip_prefix("async fn "))
                .or_else(|| t.strip_prefix("pub fn "))
                .or_else(|| t.strip_prefix("fn "))?;
            Some(sisa.split(['(', '<', ' ']).next().unwrap_or("").to_string())
        })
        .unwrap_or_default()
}

fn berkas_sumber() -> Vec<PathBuf> {
    let akar = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut out = Vec::new();
    let mut antre = vec![akar];
    while let Some(dir) = antre.pop() {
        let Ok(baca) = fs::read_dir(&dir) else { continue };
        for e in baca.flatten() {
            let p = e.path();
            if p.is_dir() {
                antre.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    assert!(!out.is_empty(), "tak menemukan satu pun berkas di src/");
    out
}

/// Apakah baris ini memanggil lapisan database secara langsung?
fn memanggil_db(kode: &str) -> bool {
    // Tanda kurung WAJIB: `repo::SesiBaru { … }` adalah struct literal, bukan
    // query, dan versi pertama pemeriksa ini menuduhnya.
    let panggilan_repo = ["repo::", "repository::"].iter().any(|p| {
        kode.split(p).skip(1).any(|sisa| {
            let nama: String = sisa.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            // Nama fungsi bergaya snake_case; tipe bergaya CamelCase.
            !nama.is_empty()
                && nama.chars().next().is_some_and(|c| c.is_lowercase())
                && sisa[nama.len()..].trim_start().starts_with('(')
        })
    });
    panggilan_repo
        || [".query(", ".query_one(", ".query_opt(", ".query_raw(", ".execute(", ".prepare(", "pool.get("]
            .iter()
            .any(|k| kode.contains(k))
}

#[test]
fn tak_ada_query_di_dalam_loop() {
    let mut temuan: Vec<String> = Vec::new();

    for path in berkas_sumber() {
        let src = fs::read_to_string(&path).unwrap_or_default();
        let nama = path
            .strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();
        let baris: Vec<&str> = src.lines().collect();

        // Tumpukan loop yang sedang terbuka: (kedalaman kurung saat masuk,
        // nomor baris, apakah ini penjadwal).
        let mut tumpukan: Vec<(i32, usize, bool)> = Vec::new();
        let mut dalam = 0i32;

        for (i, l) in baris.iter().enumerate() {
            // Komentar dibuang: berkas ini sendiri menyebut `.execute(` dalam
            // penjelasannya, dan penjelasan bukan kode.
            let kode = match l.find("//") {
                Some(k) => &l[..k],
                None => l,
            };
            let buka = kode.matches('{').count() as i32;
            let tutup = kode.matches('}').count() as i32;

            let awal_loop = kode.contains("for ") && kode.contains(" in ")
                || kode.trim_start().starts_with("while ")
                || kode.trim_start().starts_with("loop {");

            if awal_loop && buka > tutup {
                // Penjadwal dikenali dari `.tick().await` dalam ~4 baris berikutnya.
                let penjadwal = baris[i..(i + 4).min(baris.len())]
                    .iter()
                    .any(|b| b.contains(".tick()"));
                tumpukan.push((dalam + buka - tutup, i + 1, penjadwal));
            } else if let Some((_, mulai, penjadwal)) = tumpukan.last() {
                if !penjadwal && memanggil_db(kode) {
                    let fungsi = fungsi_di_baris(&baris, i);
                    let dikecualikan = DIKECUALIKAN.iter().any(|(berkas, f, _)| {
                        nama.trim_start_matches('/').ends_with(berkas.trim_start_matches("src/"))
                            && *f == fungsi
                    });
                    if !dikecualikan {
                        temuan.push(format!(
                            "  {nama}:{}  (loop dibuka di baris {mulai}, fungsi `{fungsi}`)\n       {}",
                            i + 1,
                            l.trim()
                        ));
                    }
                }
            }

            dalam += buka - tutup;
            while tumpukan.last().is_some_and(|(d, _, _)| dalam < *d) {
                tumpukan.pop();
            }
        }
    }

    assert!(
        temuan.is_empty(),
        "{} query di dalam loop — DILARANG.\n\
         Kumpulkan dulu masukannya, lalu kirim SATU query set-based \
         (`unnest(…)` / `= ANY($1::bigint[])`); pola itu sudah dipakai di \
         puluhan tempat di src/repository.\n{}",
        temuan.len(),
        temuan.join("\n")
    );
}
