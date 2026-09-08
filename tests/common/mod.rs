//! tests/common/mod.rs — Pembacaan & pemenggalan SQL dari `src/repository`,
//! dipakai bersama oleh dua pemeriksa yang saling melengkapi:
//!
//!   • `sql_repository.rs` — keselarasan SQL ↔ Rust (jumlah kolom vs `r.get(n)`,
//!     `$n` vs parameter). Tanpa database, selalu jalan.
//!   • `skema_postgres.rs` — keselarasan SQL ↔ SKEMA sungguhan (nama tabel &
//!     kolom benar-benar ada). Butuh Postgres; dilewati bila tak ada.
//!
//! Keduanya harus membaca SQL yang SAMA. Sebelum berkas ini ada, satu-satunya
//! salinan pembacanya tinggal di `sql_repository.rs` — dan pemeriksa kedua yang
//! menyalinnya akan pelan-pelan menyimpang, sampai keduanya memeriksa dua
//! himpunan query yang berbeda tanpa ada yang menyadarinya.

// Tiap pemeriksa memakai sebagian saja dari isi berkas ini; yang tak terpakai
// di salah satunya bukan kode mati, ia terpakai di sebelah.
#![allow(dead_code)]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

// ── Pembacaan sumber ─────────────────────────────────────────────────────────

pub fn berkas_repository() -> Vec<PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/repository");
    let mut out: Vec<PathBuf> = fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("gagal membaca {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "rs"))
        .collect();
    out.sort();
    assert!(!out.is_empty(), "tak menemukan satu pun berkas di src/repository");
    out
}

/// Buang isi `#[cfg(test)] mod tests { … }` — SQL contoh di dalam tes tak perlu
/// tunduk pada aturan ini, dan menyertakannya hanya melahirkan alarm palsu.
pub fn tanpa_blok_tes(src: &str) -> String {
    match src.find("#[cfg(test)]") {
        Some(i) => src[..i].to_string(),
        None => src.to_string(),
    }
}

// ── Pemenggalan literal string Rust ──────────────────────────────────────────

/// Semua literal string di `src`, beserta posisi awalnya. Escape dihormati agar
/// `\"` di tengah SQL tak dikira penutup.
pub fn literal_string(src: &str) -> Vec<(usize, String)> {
    let b = src.as_bytes();
    let (mut out, mut i) = (Vec::new(), 0usize);
    while i < b.len() {
        // Lewati komentar baris — `//` kerap memuat contoh SQL.
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        if b[i] != b'"' {
            i += 1;
            continue;
        }
        let awal = i;
        i += 1;
        let mut isi = String::new();
        while i < b.len() && b[i] != b'"' {
            if b[i] == b'\\' && i + 1 < b.len() {
                // `\` di ujung baris = sambungan; sisanya escape biasa.
                isi.push(if b[i + 1] == b'\n' { ' ' } else { b[i + 1] as char });
                i += 2;
                continue;
            }
            isi.push(b[i] as char);
            i += 1;
        }
        i += 1;
        out.push((awal, isi));
    }
    out
}

/// Rapatkan spasi berlebih supaya pola SQL mudah dicocokkan.
pub fn rapikan(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Semua `const NAMA: &str = "…";` di `src/repository`, untuk menyulih `{NAMA}`
/// pada query yang dirakit `format!`.
pub fn konstanta_sql() -> HashMap<String, String> {
    let mut out = HashMap::new();
    for path in berkas_repository() {
        let src = fs::read_to_string(&path).unwrap();
        for (pos, isi) in literal_string(&src) {
            let Some(eq) = src[..pos].rfind('=') else { continue };
            let kepala = src[..eq].trim_end();
            let Some(k) = kepala.rfind("const ") else { continue };
            let nama = kepala[k + "const ".len()..].split(':').next().unwrap_or("").trim();
            // Hanya nama bergaya konstanta, dan hanya yang berdempetan dengan
            // literalnya — supaya `const` lain di berkas yang sama tak terpungut.
            if !nama.is_empty()
                && nama.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
                && src[eq..pos].trim() == "="
            {
                out.insert(nama.to_string(), rapikan(&isi));
            }
        }
    }
    out
}

pub fn sql_beneran(s: &str) -> bool {
    let t = s.trim_start().to_ascii_uppercase();
    ["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH "]
        .iter()
        .any(|k| t.starts_with(k))
}

// ── Penyulihan `{…}` ─────────────────────────────────────────────────────────

/// Ganti setiap `{NAMA}` dengan isi konstantanya. `None` bila ada `{…}` yang
/// tak dikenali — mis. `{kelas}` yang diisi hasil pemanggilan fungsi saat jalan.
///
/// Ini SATU-SATUNYA sikap yang aman untuk pemeriksa skema: SQL yang belum utuh
/// tak bisa dikirim ke Postgres, dan menebak isinya akan melahirkan tuduhan
/// palsu. Melewatkan sesuatu boleh; menuduh yang benar tidak.
pub fn sulih_konstanta(sql: &str, konstanta: &HashMap<String, String>) -> Option<String> {
    let mut hasil = String::with_capacity(sql.len());
    let mut sisa = sql;
    while let Some(i) = sisa.find('{') {
        let j = sisa[i..].find('}')?;
        let nama = &sisa[i + 1..i + j];
        hasil.push_str(&sisa[..i]);
        hasil.push_str(konstanta.get(nama)?);
        sisa = &sisa[i + j + 1..];
    }
    hasil.push_str(sisa);
    Some(hasil)
}

/// Seluruh query di `src/repository` yang bentuknya sudah UTUH — siap dikirim
/// ke Postgres. Return `(nama_berkas, sql)`.
pub fn query_utuh() -> Vec<(String, String)> {
    let konstanta = konstanta_sql();
    let mut out = Vec::new();
    for path in berkas_repository() {
        let nama = path.file_name().unwrap().to_string_lossy().to_string();
        let src = tanpa_blok_tes(&fs::read_to_string(&path).unwrap());
        for (_, isi) in literal_string(&src) {
            let sql = rapikan(&isi);
            // Penyulihan DULU, baru diuji apakah ini SQL. Query yang ditulis
            // `format!("{KOLOM_SELECT} WHERE …")` diawali `{`; menguji lebih
            // dulu akan membuangnya, dan justru query bentuk itulah yang paling
            // perlu diperiksa — daftar kolomnya dipakai bersama beberapa fungsi.
            let Some(utuh) = sulih_konstanta(&sql, &konstanta) else {
                continue;
            };
            if sql_beneran(&utuh) {
                out.push((nama.clone(), utuh));
            }
        }
    }
    out
}
