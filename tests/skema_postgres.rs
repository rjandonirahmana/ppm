//! tests/skema_postgres.rs — Setiap query di `src/repository` diadu dengan
//! SKEMA SUNGGUHAN.
//!
//! ── LUBANG YANG DITUTUPNYA ───────────────────────────────────────────────────
//! `sql_repository.rs` memeriksa keselarasan SQL ↔ Rust tanpa database, dan
//! catatan di kepalanya menyebut sendiri apa yang TIDAK bisa ia lihat:
//!
//!   > nama kolom salah ketik, tipe yang tak cocok, atau tabel yang sudah
//!   > dibuang tetap lolos. Untuk itu perlu Postgres sungguhan.
//!
//! Selama ini "Postgres sungguhan" tak pernah ada di jalur mana pun: NOL tes di
//! proyek ini menyentuh database (satu-satunya yang membuat pool sengaja
//! menunjuk port mati), dan 93 migrasi diterapkan dengan tangan. Akibatnya
//! ketidakcocokan antara query dan skema baru ketahuan sebagai galat 500 pada
//! permintaan sungguhan, di produksi.
//!
//! ── CARA KERJANYA ────────────────────────────────────────────────────────────
//! Tiap SQL dikirim ke Postgres lewat `Client::prepare` — itu PREPARE sisi
//! server: pernyataannya di-parse dan DIANALISIS penuh (tabel, kolom, fungsi,
//! tipe) tapi TIDAK dijalankan. Tak ada baris yang dibaca, ditulis, atau
//! dihapus, jadi tes ini aman dijalankan berkali-kali atas skema kosong —
//! bahkan `DELETE FROM users` sekalipun cuma diperiksa bentuknya.
//!
//! ── KENAPA SEBAGIAN GALAT JUSTRU DIANGGAP LULUS ──────────────────────────────
//! `42P18 indeterminate_datatype` ("could not determine data type of parameter")
//! BUKAN kesalahan skema. Ia muncul saat Postgres tak bisa menyimpulkan tipe
//! sebuah `$n` dari konteksnya — mis. `COALESCE($1, …)`. Untuk sampai ke
//! kesimpulan itu, seluruh nama tabel dan kolom di pernyataan tersebut SUDAH
//! diselesaikan lebih dulu; yang tersisa hanya soal tipe parameter, dan itu
//! ditentukan Rust saat memanggil, bukan oleh skema. Menjadikannya kegagalan
//! akan membuat tes ini menuduh query yang benar — dan pemeriksa yang salah
//! tuduh akan dimatikan orang, sesudah itu ia tak menangkap apa pun lagi.
//!
//! Yang DIANGGAP GAGAL justru yang menunjuk skema secara langsung:
//! `42P01` tabel tak ada, `42703` kolom tak ada, `42883` fungsi tak ada,
//! `42601` sintaks salah, `42P10`/`42704` acuan lain yang tak ditemukan.
//!
//! ── DILEWATI BILA TAK ADA DATABASE ───────────────────────────────────────────
//! Tanpa `TEST_DATABASE_URL`, tes ini lulus tanpa memeriksa apa pun. Itu
//! disengaja: `cargo test` di laptop tak boleh menuntut Postgres yang skemanya
//! sudah termigrasi, sementara di CI variabelnya selalu ada (lihat job `test`
//! di .github/workflows/master.yml, yang menjalankan scripts/migrate.sh lebih
//! dulu). Bila suatu saat CI berhenti menyetelnya, tes ini akan diam-diam
//! berhenti bekerja — karena itu ia MELAPOR di stdout apa yang ia lakukan.

mod common;

/// SQLSTATE yang benar-benar menandakan query dan skema tak sepakat.
const GALAT_SKEMA: &[(&str, &str)] = &[
    ("42P01", "tabel/view tak ada"),
    ("42703", "kolom tak ada"),
    ("42883", "fungsi/operator tak ada"),
    ("42601", "sintaks SQL salah"),
    ("42704", "objek tak ditemukan"),
    ("42P10", "acuan kolom tak sah"),
];

#[tokio::test(flavor = "current_thread")]
async fn query_repository_cocok_dengan_skema() {
    let Ok(url) = std::env::var("TEST_DATABASE_URL") else {
        println!(
            "TEST_DATABASE_URL tak diset — pemeriksaan skema DILEWATI. \
             Di CI variabel ini selalu ada; kalau pesan ini muncul di sana, \
             job `test` berhenti memeriksa skema."
        );
        return;
    };

    let (client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls)
        .await
        .unwrap_or_else(|e| panic!("gagal menyambung ke TEST_DATABASE_URL: {e}"));
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("koneksi Postgres putus: {e}");
        }
    });

    // Skema harus SUDAH termigrasi — kalau tidak, seluruh query akan dilaporkan
    // salah dan pesannya menyesatkan. Diperiksa lebih dulu, dengan kalimat yang
    // menyebut apa yang harus dijalankan.
    let ada_users: bool = client
        .query_one("SELECT to_regclass('public.users') IS NOT NULL", &[])
        .await
        .expect("gagal memeriksa skema")
        .get(0);
    assert!(
        ada_users,
        "tabel `users` tak ada — database di TEST_DATABASE_URL belum dimigrasi. \
         Jalankan `DATABASE_URL=… ./scripts/migrate.sh up` lebih dulu."
    );

    let query = common::query_utuh();
    assert!(
        query.len() > 100,
        "hanya {} query yang berhasil dibaca dari src/repository — pembacanya \
         kemungkinan rusak, dan tes ini jadi lulus tanpa memeriksa apa pun",
        query.len()
    );

    let mut salah: Vec<String> = Vec::new();
    let mut diperiksa = 0usize;
    let mut dilewati_tipe = 0usize;

    for (berkas, sql) in &query {
        match client.prepare(sql).await {
            Ok(_) => diperiksa += 1,
            Err(e) => {
                let kode = e.code().map(|c| c.code().to_string()).unwrap_or_default();
                match GALAT_SKEMA.iter().find(|(k, _)| *k == kode) {
                    Some((_, arti)) => {
                        let pesan = e
                            .as_db_error()
                            .map(|d| d.message().to_string())
                            .unwrap_or_else(|| e.to_string());
                        // Query dipotong: yang menolong pembaca adalah pesan
                        // Postgres-nya, bukan 600 karakter SQL di terminal.
                        let cuplik: String = sql.chars().take(160).collect();
                        salah.push(format!("  {berkas} [{kode} {arti}] {pesan}\n     → {cuplik}…"));
                    }
                    // 42P18 dan kerabatnya: bukan urusan skema — lihat catatan modul.
                    None => dilewati_tipe += 1,
                }
            }
        }
    }

    println!(
        "skema: {diperiksa} query cocok, {dilewati_tipe} dilewati (tipe parameter tak tentu), \
         {} bermasalah",
        salah.len()
    );

    assert!(
        salah.is_empty(),
        "{} query tak cocok dengan skema:\n{}",
        salah.len(),
        salah.join("\n")
    );
}
