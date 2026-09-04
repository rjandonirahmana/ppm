//! tests/notifikasi_skema.rs — Kontrak antara `migration/92_notifikasi.sql` dan
//! kode yang membacanya.
//!
//! ── KENAPA TES INI ADA ───────────────────────────────────────────────────────
//! Migrasi di proyek ini dijalankan MANUAL (`scripts/migrate.sh up`), terpisah
//! dari kompilasi. Artinya SQL dan Rust bisa menyimpang tanpa satu pun peringatan:
//! kolom yang diubah namanya di migrasi tetap lolos `cargo check`, dan barisnya
//! baru meledak saat ada santri yang mengajukan izin.
//!
//! `tests/sql_repository.rs` sudah menjaga keselarasan POSISI kolom (`$n` ↔
//! parameter, `r.get(n)` ↔ jumlah kolom SELECT). Yang TIDAK dijaganya adalah
//! apakah kolom & index yang diandalkan itu benar-benar ada di migrasinya.
//! Itulah yang diperiksa di sini — tanpa database, jadi ia ikut jalan di mana
//! pun `cargo test` dijalankan.
//!
//! ── APA YANG *TIDAK* DIPERIKSA ───────────────────────────────────────────────
//! Ini pemeriksaan TEKS atas berkas migrasi, bukan atas database sungguhan.
//! Migrasi yang belum dijalankan di server tetap lolos di sini. Yang ditangkap
//! adalah penyimpangan antara dua berkas di repo yang sama — bukan pengganti
//! menjalankan migrasinya.

use std::fs;

fn migrasi() -> String {
    fs::read_to_string("migration/92_notifikasi.sql")
        .expect("migration/92_notifikasi.sql hilang — notifikasi tak akan punya tabelnya")
}

fn repo() -> String {
    fs::read_to_string("src/repository/notifications.rs")
        .expect("src/repository/notifications.rs hilang")
}

#[test]
fn tabel_notifications_dibuat() {
    let m = migrasi();
    assert!(
        m.contains("CREATE TABLE IF NOT EXISTS notifications"),
        "migrasi 92 harus membuat tabel `notifications`"
    );
}

/// Tiap kolom yang dibaca repository harus ada di migrasinya.
///
/// Daftar ini sengaja ditulis ulang, bukan diturunkan dari kode: dua salinan
/// yang harus disepakati manusia justru itulah gunanya — perubahan sepihak di
/// salah satunya berhenti di sini alih-alih di produksi.
#[test]
fn kolom_yang_dibaca_repository_ada_di_migrasi() {
    let m = migrasi();
    for kolom in ["id", "user_id", "kind", "title", "body", "link", "read_at", "created_at"] {
        assert!(
            m.contains(kolom),
            "kolom `{kolom}` dibaca repository tapi tak ada di migrasi 92"
        );
    }
}

/// `ON DELETE CASCADE` bukan detail gaya.
///
/// Tanpanya, menghapus akun akan GAGAL selama masih ada notifikasi miliknya —
/// dan yang tampil di layar admin adalah galat foreign key yang tak menyebut
/// notifikasi sama sekali.
#[test]
fn notifikasi_ikut_terhapus_bersama_akunnya() {
    let m = migrasi();
    assert!(
        m.contains("REFERENCES users(id) ON DELETE CASCADE"),
        "user_id harus ON DELETE CASCADE — lihat catatan di migrasi"
    );
}

/// Index feed: `(user_id, created_at DESC)`.
///
/// Urutan kolomnya harus persis begitu supaya query feed jadi index scan yang
/// berhenti setelah LIMIT baris, bukan sort atas seluruh notifikasi seseorang.
#[test]
fn index_feed_ada_dan_urutannya_benar() {
    let m = migrasi();
    assert!(
        m.contains("ON notifications (user_id, created_at DESC)"),
        "index feed `(user_id, created_at DESC)` hilang atau urutannya berubah"
    );
}

/// Index PARSIAL untuk penghitung lonceng.
///
/// Ini query terpanas di tabel ini — dijalankan tiap pemuatan halaman oleh
/// setiap orang yang sedang masuk. Tanpa `WHERE read_at IS NULL`, indexnya ikut
/// memuat seluruh riwayat yang sudah dibaca dan tumbuh selamanya, padahal yang
/// dihitung selalu himpunan kecil.
#[test]
fn index_belum_dibaca_bersifat_parsial() {
    let m = migrasi();
    let punya_parsial = m
        .lines()
        .skip_while(|l| !l.contains("idx_notifications_belum_dibaca"))
        .take(4)
        .any(|l| l.contains("WHERE read_at IS NULL"));
    assert!(
        punya_parsial,
        "idx_notifications_belum_dibaca harus PARSIAL (`WHERE read_at IS NULL`)"
    );
}

/// Penulisan banyak penerima harus tetap satu perjalanan ke database.
///
/// Satu pengajuan izin memberi tahu wali kelas DAN semua admin. Kalau ini
/// diam-diam kembali jadi loop `INSERT`, biayanya bertambah satu round-trip per
/// admin — dan penulisannya berhenti atomik, sehingga bisa berakhir dengan wali
/// kelas dapat notifikasi sementara admin tidak.
#[test]
fn penulisan_massal_memakai_unnest() {
    let r = repo();
    assert!(
        r.contains("UNNEST"),
        "notif_insert_many harus memakai UNNEST, bukan loop INSERT"
    );
}

/// Kepemilikan ditegakkan di QUERY, bukan dipercayakan ke pemanggil.
///
/// Tanpa `user_id` di WHERE, siapa pun yang menebak nomor bisa menandai
/// notifikasi orang lain terbaca. Ini satu-satunya pagar yang mencegahnya —
/// server function di atasnya sengaja hanya menuntut login (lihat
/// `tests/wewenang_api.rs::notifikasi_dijaga_login_bukan_peran`).
#[test]
fn tandai_terbaca_menyaring_pemiliknya() {
    let r = repo();
    let blok = r
        .split("pub async fn notif_mark_read")
        .nth(1)
        .expect("notif_mark_read hilang");
    let blok = &blok[..blok.find("\npub ").unwrap_or(blok.len())];
    assert!(
        blok.contains("user_id = $2"),
        "notif_mark_read harus menyaring user_id di WHERE, bukan hanya id"
    );
}

/// Menandai terbaca harus idempoten: mengetuk dua kali tak boleh menggeser
/// waktu bacanya, karena waktu itu ikut dipakai membaca urutan kejadian.
#[test]
fn tandai_terbaca_idempoten() {
    let r = repo();
    assert!(
        r.contains("AND read_at IS NULL"),
        "UPDATE tandai-terbaca harus dibatasi `read_at IS NULL`"
    );
}
