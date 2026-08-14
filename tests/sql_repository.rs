//! tests/sql_repository.rs — Pemeriksa keselarasan SQL ↔ Rust di `src/repository`.
//!
//! ── KENAPA TES INI ADA ───────────────────────────────────────────────────────
//! Lapis repository adalah satu-satunya bagian proyek ini yang kompilator TIDAK
//! bisa periksa. SQL diikat ke Rust lewat POSISI, bukan nama:
//!
//!   • `r.get(17)` bertipe dinamis — salah indeks baru ketahuan saat query jalan;
//!   • `$16` cuma teks di dalam string — jumlahnya tak dicocokkan dengan
//!     parameter yang benar-benar dikirim.
//!
//! Dua kerusakan produksi pada 14 Agustus 2026 lahir dari persis dua hal itu,
//! keduanya lolos `cargo check` DAN 143 tes yang sudah ada:
//!
//!   1. satu kolom dibuang dari SELECT → seluruh `r.get(n)` sesudahnya bergeser,
//!      dan `class_schedules` panik di setiap pemuatan halaman;
//!   2. satu parameter dibuang dari INSERT → `$16` tertinggal tanpa pasangan,
//!      dan setiap pembuatan jadwal pasti gagal.
//!
//! Keduanya berbentuk sama: kolom/parameter disunting di satu tempat, pasangannya
//! tertinggal di tempat lain. Bentuk itulah yang diperiksa di sini — tanpa
//! database, jadi ia ikut jalan di `cargo test` mana pun.
//!
//! ── APA YANG *TIDAK* DIPERIKSA ───────────────────────────────────────────────
//! Tes ini TIDAK memvalidasi SQL ke skema sungguhan: nama kolom salah ketik,
//! tipe yang tak cocok, atau tabel yang sudah dibuang tetap lolos. Untuk itu
//! perlu Postgres sungguhan. Yang ada di sini adalah jaring pertama yang murah
//! dan selalu jalan — bukan pengganti tes integrasi.

use std::fs;
use std::path::{Path, PathBuf};

// ── Pembacaan sumber ─────────────────────────────────────────────────────────

fn berkas_repository() -> Vec<PathBuf> {
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
fn tanpa_blok_tes(src: &str) -> String {
    match src.find("#[cfg(test)]") {
        Some(i) => src[..i].to_string(),
        None => src.to_string(),
    }
}

// ── Pemenggalan literal string Rust ──────────────────────────────────────────

/// Semua literal string di `src`, beserta posisi awalnya. Escape dihormati agar
/// `\"` di tengah SQL tak dikira penutup.
fn literal_string(src: &str) -> Vec<(usize, String)> {
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
fn rapikan(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn sql_beneran(s: &str) -> bool {
    let t = s.trim_start().to_ascii_uppercase();
    ["SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH "]
        .iter()
        .any(|k| t.starts_with(k))
}

// ── Pencacah ─────────────────────────────────────────────────────────────────

/// Cacah elemen teratas yang dipisahkan koma — koma di dalam kurung diabaikan.
///
/// KOMA DI UJUNG DIBUANG DULU. Rust membolehkannya (`&[&a, &b,]`) dan gaya
/// rustfmt malah menambahkannya pada daftar multi-baris — tanpa langkah ini
/// SETIAP query multi-baris terhitung kelebihan satu parameter, dan pemeriksa
/// ini melapor 14 alarm palsu pada percobaan pertamanya.
fn cacah_elemen(s: &str) -> usize {
    let s = s.trim().trim_end_matches(',').trim();
    if s.is_empty() {
        return 0;
    }
    let (mut dalam, mut n) = (0i32, 1usize);
    for c in s.chars() {
        match c {
            '(' | '[' | '{' => dalam += 1,
            ')' | ']' | '}' => dalam -= 1,
            ',' if dalam == 0 => n += 1,
            _ => {}
        }
    }
    n
}

/// Jumlah kolom teratas sebuah `SELECT … FROM`. `None` bila bentuknya bukan itu
/// (mis. `INSERT`, atau `SELECT` tanpa `FROM`).
fn kolom_select(sql: &str) -> Option<usize> {
    let atas = sql.to_ascii_uppercase();
    let mulai = atas.find("SELECT ")? + "SELECT ".len();
    let sisa = &sql[mulai..];
    let atas_sisa = &atas[mulai..];

    // `char_indices` memberi indeks BYTE, bukan urutan karakter. Versi pertama
    // mencacah dengan indeks karakter lalu memotong string dengan angka itu —
    // dan langsung panik pada em-dash di komentar SQL. Rust menolak potongan di
    // tengah karakter, jadi kesalahan ini selalu ketahuan; di bahasa lain ia
    // hanya menghasilkan teks rusak.
    let (mut dalam, mut akhir) = (0i32, None);
    for (i, c) in sisa.char_indices() {
        match c {
            '(' => dalam += 1,
            ')' => dalam -= 1,
            _ => {}
        }
        // `FROM` teratas — milik subquery diabaikan karena `dalam > 0`.
        if dalam == 0 && atas_sisa[i..].starts_with("FROM ") {
            akhir = Some(i);
            break;
        }
    }
    Some(cacah_elemen(&sisa[..akhir?]))
}

/// Semua `$n` yang muncul di SQL.
fn placeholder(sql: &str) -> Vec<usize> {
    let b: Vec<char> = sql.chars().collect();
    let (mut out, mut i) = (Vec::new(), 0usize);
    while i < b.len() {
        if b[i] == '$' {
            let mut j = i + 1;
            let mut n = String::new();
            while j < b.len() && b[j].is_ascii_digit() {
                n.push(b[j]);
                j += 1;
            }
            if let Ok(v) = n.parse::<usize>() {
                out.push(v);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    out
}

/// Semua indeks `r.get(N)` / `row.get::<_, T>(N)` di sepotong kode.
fn indeks_get(kode: &str) -> Vec<usize> {
    let mut out = Vec::new();
    for (i, _) in kode.match_indices(".get") {
        let sisa = &kode[i + 4..];
        // lewati turbofish `::<…>`
        let sisa = match sisa.strip_prefix("::<") {
            Some(s) => match s.find('>') {
                Some(k) => &s[k + 1..],
                None => continue,
            },
            None => sisa,
        };
        let Some(sisa) = sisa.strip_prefix('(') else { continue };
        let angka: String = sisa.chars().take_while(|c| c.is_ascii_digit()).collect();
        if angka.is_empty() {
            continue;
        }
        if !sisa[angka.len()..].starts_with(')') {
            continue;
        }
        if let Ok(v) = angka.parse() {
            out.push(v);
        }
    }
    out
}

/// Isi slice parameter `&[ … ]` pertama sesudah `dari`, bila ada.
fn slice_parameter(kode: &str) -> Option<&str> {
    let i = kode.find("&[")?;
    let mut dalam = 0i32;
    // Indeks BYTE — lihat catatan di `kolom_select`.
    for (j, c) in kode[i..].char_indices() {
        match c {
            '[' => dalam += 1,
            ']' => {
                dalam -= 1;
                if dalam == 0 {
                    return Some(&kode[i + 2..i + j]);
                }
            }
            _ => {}
        }
    }
    None
}

// ── Pemeriksaan ──────────────────────────────────────────────────────────────

/// Satu query beserta potongan kode yang mengikutinya, sampai query berikutnya.
struct Petak {
    sql: String,
    ekor: String,
}

/// Pecah sumber jadi petak: tiap literal SQL memegang kode sesudahnya hingga
/// literal SQL berikutnya. Pemetaan `r.get()` dan slice parameter milik sebuah
/// query selalu ditulis tepat sesudahnya, jadi pembagian ini memasangkan
/// keduanya dengan benar — termasuk pada fungsi yang memuat DUA query.
fn petak_query(src: &str) -> Vec<Petak> {
    let lits: Vec<(usize, String)> = literal_string(src)
        .into_iter()
        .map(|(p, s)| (p, rapikan(&s)))
        .filter(|(_, s)| sql_beneran(s))
        .collect();

    let mut out = Vec::new();
    for (k, (pos, sql)) in lits.iter().enumerate() {
        // Jendela berakhir pada literal SQL berikutnya, ATAU pada pemanggilan
        // query berikutnya — mana yang lebih dulu.
        //
        // Batas kedua itu perlu: query yang dirakit `format!` sering tak punya
        // literal yang diawali SELECT, jadi tanpa batas ini jendela sebuah query
        // bocor ke pemetaan `r.get()` milik query SESUDAHNYA — dan pemeriksa
        // menuduh kode yang sebetulnya benar.
        let batas_lit = lits.get(k + 1).map(|(p, _)| *p).unwrap_or(src.len());
        let sesudah_await = src[*pos..]
            .find(".await")
            .map(|i| *pos + i)
            .unwrap_or(*pos);
        let batas_call = [".query(", ".query_one(", ".query_opt(", ".execute("]
            .iter()
            .filter_map(|k| src[sesudah_await..].find(k).map(|i| sesudah_await + i))
            .min()
            .unwrap_or(src.len());
        let batas = batas_lit.min(batas_call).max(*pos);
        let ekor = potong_di_pemetaan_kedua(&src[*pos..batas]);

        out.push(Petak {
            sql: sql.clone(),
            ekor,
        });
    }
    out
}

/// Potong jendela begitu PEMETAAN KEDUA dimulai.
///
/// Dua query yang dijalankan berbarengan (`tokio::join!`) tak menyisakan
/// pemanggilan query di antara literal pertama dan pemetaan kedua, jadi batas
/// berbasis pemanggilan tak menolong. Yang bisa diandalkan: sebuah blok
/// pemetaan hampir selalu dibuka dengan indeks 0. Kemunculan `.get(0)` yang
/// KEDUA karena itu menandai mulainya pemetaan milik query lain.
///
/// Bila satu pemetaan kebetulan membaca indeks 0 dua kali, jendelanya terpotong
/// terlalu cepat dan sebagian indeks tak diperiksa. Itu arah kesalahan yang
/// dipilih dengan sengaja — melewatkan sesuatu bisa dimaafkan, menuduh yang
/// benar tidak.
fn potong_di_pemetaan_kedua(ekor: &str) -> String {
    let mut cari = 0usize;
    let mut ketemu = 0;
    while let Some(i) = ekor[cari..].find(".get(0)") {
        let abs = cari + i;
        ketemu += 1;
        if ketemu == 2 {
            return ekor[..abs].to_string();
        }
        cari = abs + ".get(0)".len();
    }
    ekor.to_string()
}

/// Apakah jumlah kolom query ini bisa dipastikan secara statis?
///
/// TIDAK bisa, dan karena itu dilewati, bila:
///   • memuat `{…}` — daftar kolomnya dirakit `format!` saat berjalan;
///   • diawali `WITH` — SELECT teratasnya bukan yang pertama ditemukan;
///   • bukan `SELECT` (UPDATE/INSERT/DELETE) — "SELECT" yang terlihat milik
///     subquery, dan kolom keluarannya ditentukan `RETURNING`.
///
/// Melewatkan sesuatu jauh lebih baik daripada menuduhnya: pemeriksa yang
/// sering salah tuduh akan dimatikan orang, dan setelah itu ia tak menangkap
/// apa pun lagi.
fn bisa_dicacah(sql: &str) -> bool {
    let t = sql.trim_start().to_ascii_uppercase();
    t.starts_with("SELECT ") && !sql.contains('{')
}

/// Setiap `$n` harus punya parameter. Ini yang menangkap `$16` tertinggal.
#[test]
fn placeholder_cocok_dengan_jumlah_parameter() {
    let mut salah = Vec::new();
    for path in berkas_repository() {
        let src = tanpa_blok_tes(&fs::read_to_string(&path).unwrap());
        let nama = path.file_name().unwrap().to_string_lossy().to_string();
        for p in petak_query(&src) {
            let ph = placeholder(&p.sql);
            let Some(&maks) = ph.iter().max() else { continue };
            let Some(slice) = slice_parameter(&p.ekor) else { continue };
            let n = cacah_elemen(slice);
            if n != maks {
                salah.push(format!(
                    "{nama}: placeholder tertinggi ${maks} tapi {n} parameter\n    {}",
                    &p.sql[..p.sql.len().min(110)]
                ));
            }
        }
    }
    assert!(
        salah.is_empty(),
        "SQL dengan jumlah parameter tak sepadan ({}):\n  {}",
        salah.len(),
        salah.join("\n  ")
    );
}

/// `r.get(n)` tak boleh melewati jumlah kolom SELECT. Ini yang menangkap
/// pergeseran indeks saat sebuah kolom dibuang.
#[test]
fn indeks_kolom_tidak_melewati_select() {
    let mut salah = Vec::new();
    for path in berkas_repository() {
        let src = tanpa_blok_tes(&fs::read_to_string(&path).unwrap());
        let nama = path.file_name().unwrap().to_string_lossy().to_string();
        for p in petak_query(&src) {
            if !bisa_dicacah(&p.sql) {
                continue;
            }
            let Some(kol) = kolom_select(&p.sql) else { continue };
            if kol == 0 {
                continue;
            }
            let gets = indeks_get(&p.ekor);
            let Some(&maks) = gets.iter().max() else { continue };
            if maks >= kol {
                salah.push(format!(
                    "{nama}: SELECT {kol} kolom tapi membaca r.get({maks})\n    {}",
                    &p.sql[..p.sql.len().min(110)]
                ));
            }
        }
    }
    assert!(
        salah.is_empty(),
        "indeks kolom di luar batas ({}):\n  {}",
        salah.len(),
        salah.join("\n  ")
    );
}

// ── Tes untuk pemeriksanya sendiri ───────────────────────────────────────────
//
// Pemeriksa yang diam-diam berhenti bekerja lebih buruk daripada tak ada
// pemeriksa: ia memberi rasa aman tanpa memberi jaminan. Bagian ini memastikan
// ia benar-benar MELIHAT kedua bentuk bug yang menjadi alasan keberadaannya —
// dan tidak menuduh kode yang benar.

#[test]
fn pemeriksa_mengenali_kolom_yang_bergeser() {
    // Bentuk persis bug `class_schedules`: kolom dibuang, indeks tak digeser.
    let contoh = r#"
        let rows = c.query("SELECT a, b, c FROM t WHERE id = $1", &[&id]).await?;
        rows.into_iter().map(|r| X { a: r.get(0), c: r.get(3) }).collect()
    "#;
    let p = &petak_query(contoh)[0];
    assert_eq!(kolom_select(&p.sql), Some(3));
    assert_eq!(indeks_get(&p.ekor).into_iter().max(), Some(3), "3 >= 3 → harus tertangkap");
}

#[test]
fn pemeriksa_mengenali_placeholder_yatim() {
    // Bentuk persis bug `create_schedule`: $4 tanpa parameter keempat.
    let contoh = r#"
        c.execute("INSERT INTO t (a,b,c) VALUES ($1, $2, $3, $4)", &[&a, &b, &c]).await?;
    "#;
    let p = &petak_query(contoh)[0];
    assert_eq!(placeholder(&p.sql).into_iter().max(), Some(4));
    assert_eq!(cacah_elemen(slice_parameter(&p.ekor).unwrap()), 3);
}

/// Koma di ujung daftar parameter tak boleh dihitung sebagai elemen — gaya
/// rustfmt selalu menambahkannya pada daftar multi-baris.
#[test]
fn koma_di_ujung_tidak_dihitung() {
    assert_eq!(cacah_elemen("&a, &b, &c"), 3);
    assert_eq!(cacah_elemen("&a, &b, &c,"), 3);
    assert_eq!(cacah_elemen("\n    &a,\n    &b,\n"), 2);
    assert_eq!(cacah_elemen(""), 0);
    assert_eq!(cacah_elemen("  "), 0);
}

#[test]
fn pemeriksa_tidak_menuduh_kode_yang_benar() {
    let contoh = r#"
        let rows = c.query("SELECT a, b, c FROM t WHERE id = $1", &[&id]).await?;
        rows.into_iter().map(|r| X { a: r.get(0), b: r.get(1), c: r.get(2) }).collect()
    "#;
    let p = &petak_query(contoh)[0];
    assert_eq!(kolom_select(&p.sql), Some(3));
    assert_eq!(indeks_get(&p.ekor).into_iter().max(), Some(2)); // 2 < 3 → aman
    assert_eq!(placeholder(&p.sql).into_iter().max(), Some(1));
    assert_eq!(cacah_elemen(slice_parameter(&p.ekor).unwrap()), 1);
}

/// Koma di dalam kurung BUKAN pemisah kolom — subquery dan `COALESCE(a, b)`
/// akan membuat cacahnya meleset kalau ini salah, dan setiap query rumit
/// berubah jadi alarm palsu.
#[test]
fn cacah_kolom_mengabaikan_koma_dalam_kurung() {
    assert_eq!(
        kolom_select("SELECT id, COALESCE(a, b, c), (SELECT x FROM y LIMIT 1) FROM t"),
        Some(3)
    );
    assert_eq!(
        kolom_select("SELECT CASE WHEN a THEN 1 ELSE 2 END, b FROM t"),
        Some(2)
    );
}

/// Pemetaan query kedua tak boleh dihitung sebagai milik query pertama.
#[test]
fn pemetaan_kedua_dipotong() {
    let ekor = "map(|r| A { x: r.get(0), y: r.get(1) }) \
                map(|r| B { p: r.get(0), q: r.get(4) })";
    let potong = potong_di_pemetaan_kedua(ekor);
    assert_eq!(indeks_get(&potong).into_iter().max(), Some(1), "get(4) milik query lain");
}

/// Query yang kolomnya dirakit `format!` tak bisa dicacah — harus dilewati,
/// bukan ditebak.
#[test]
fn query_dengan_format_dilewati() {
    assert!(!bisa_dicacah("SELECT {BOOK_COLS} FROM books WHERE id = $1"));
    assert!(!bisa_dicacah("WITH x AS (SELECT 1) SELECT a, b FROM x"));
    assert!(!bisa_dicacah("UPDATE t SET a = $1 WHERE id IN (SELECT id FROM u)"));
    assert!(bisa_dicacah("SELECT a, b FROM t"));
}

/// SQL di proyek ini memuat em-dash dan tanda kutip melengkung di komentarnya.
/// Memotong string di tengah karakter multi-byte membuat Rust panik — dan
/// pemeriksa yang panik sama tak bergunanya dengan pemeriksa yang tak ada.
#[test]
fn karakter_multibyte_tidak_membuat_panik() {
    assert_eq!(kolom_select("SELECT a, b FROM t -- catatan — dgn em-dash"), Some(2));
    assert_eq!(kolom_select("SELECT 'ā', b, c FROM t"), Some(3));
    assert_eq!(slice_parameter("&[&a, &b] // “kutip melengkung”"), Some("&a, &b"));
}

/// `FROM` milik subquery tak boleh dikira `FROM` utama.
#[test]
fn from_subquery_tidak_mengakhiri_daftar_kolom() {
    assert_eq!(
        kolom_select("SELECT a, (SELECT n FROM lain WHERE z = 1), b FROM utama"),
        Some(3)
    );
}

/// Dua query dalam satu fungsi harus dipasangkan dengan parameternya
/// masing-masing — inilah yang membuat versi awal pemeriksa ini penuh alarm
/// palsu, dan alasan pemetakan dilakukan per-query, bukan per-fungsi.
#[test]
fn dua_query_sefungsi_dipasangkan_terpisah() {
    let contoh = r#"
        c.execute("UPDATE t SET a = $2 WHERE id = $1", &[&id, &a]).await?;
        c.execute("DELETE FROM t WHERE id = $1", &[&id]).await?;
    "#;
    let petak = petak_query(contoh);
    assert_eq!(petak.len(), 2);
    assert_eq!(placeholder(&petak[0].sql).into_iter().max(), Some(2));
    assert_eq!(cacah_elemen(slice_parameter(&petak[0].ekor).unwrap()), 2);
    assert_eq!(placeholder(&petak[1].sql).into_iter().max(), Some(1));
    assert_eq!(cacah_elemen(slice_parameter(&petak[1].ekor).unwrap()), 1);
}
