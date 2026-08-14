//! repository — Query Postgres (server-only), dipisah per-domain.
//! Baris → struct sederhana; pemformatan tampilan dilakukan di layer service.

pub mod activity_log;
pub mod activity_photos;
pub mod articles;
pub mod attendance;
pub mod books;
pub mod device;
pub mod finance;
pub mod gate;
pub mod guest;
pub mod hafalan;
pub mod kelas;
pub mod materials;
pub mod parents;
pub mod permits;
pub mod schedule;
pub mod semester;
pub mod users;

pub use activity_log::*;
pub use activity_photos::*;
pub use articles::*;
pub use attendance::*;
pub use books::*;
pub use device::*;
pub use finance::*;
pub use gate::*;
pub use guest::*;
pub use hafalan::*;
pub use kelas::*;
pub use materials::*;
pub use parents::*;
pub use permits::*;
pub use schedule::*;
pub use semester::*;
pub use users::*;

/// Kelas AKADEMIK seorang santri — dipakai di mana pun satu nama kelas perlu
/// mewakilinya (rekap, papan poin, tagihan, fallback izin).
///
/// Kenapa ini ada: satu santri terdaftar di BANYAK kelas sekaligus — KBM,
/// piket harian, apel, sholat. Kolom `class_participants.is_primary` dulu
/// dipakai memilih "kelas utama", tapi tak pernah ada yang mengisinya (nol
/// baris bertanda primary di produksi), sehingga setiap join yang bersandar
/// padanya menghasilkan NULL dan nama kelas hilang dari laporan.
///
/// Sejak migrasi 65 pertanyaannya punya jawaban langsung: kelas KBM. Satu
/// santri paling banyak punya SATU (dijaga trigger `trg_satu_kelas_kbm`), jadi
/// ini bukan lagi "yang paling mungkin" melainkan memang kelasnya. Sebelumnya
/// terpaksa menebak lewat `golongan IN ('bacaan','makna')` — sumbu klasifikasi
/// lama yang di produksi tak pernah terisi seperti yang dibayangkan.
///
/// Santri tanpa kelas KBM (mis. baru masuk, baru ikut piket) jatuh ke kelas
/// mana pun, diurut `id` supaya hasilnya tetap sama tiap kali dibaca.
///
/// Pemakaian: sisipkan sebagai LATERAL, ganti `{U}` dengan kolom user id.
/// Aliasnya `cl`.
///
/// Kolomnya disebut satu per satu, bukan `c.*`: lateral ini menempel di
/// belasan query (rekap, papan poin, tagihan, izin) dan `classes` punya kolom
/// teks panjang (`description`) serta kolom yang terus bertambah tiap migrasi.
/// Yang dibaca pemanggil hanya sembilan di bawah — sisanya biaya transfer
/// murni. Menambah kolom baru ke `classes` juga jadi tak diam-diam memperbesar
/// setiap query ini.
/// Predikat "`kolom` jatuh pada HARI INI menurut WIB" — bentuk yang masih bisa
/// memakai index.
///
/// Bentuk yang tampak paling wajar justru yang paling mahal:
/// `(kolom AT TIME ZONE 'Asia/Jakarta')::date = (NOW() AT TIME ZONE ...)::date`.
/// Begitu kolomnya dibungkus fungsi, planner tak bisa lagi memakai btree di
/// atasnya — dan index ekspresi pun mustahil dibuat karena ekspresinya STABLE,
/// bukan IMMUTABLE. Hasilnya seq scan atas seluruh tabel, dijalankan setiap
/// kali dashboard guru/admin dibuka, pada tabel yang tumbuh ±1 juta
/// baris setahun (proyeksi migrasi 68).
///
/// Di sini kolomnya dibiarkan telanjang di kiri dan yang dihitung adalah BATAS
/// harinya: tengah malam WIB hari ini, dan tengah malam berikutnya. Keduanya
/// ekspresi konstan untuk satu query, jadi planner bisa mengubahnya menjadi
/// pemindaian rentang.
///
/// Batas atasnya `<`, bukan `<=`: satu detik sebelum tengah malam berikutnya
/// masih hari ini, tengah malamnya sendiri sudah besok.
///
/// Pemakaian: `format!("WHERE ... AND {}", hari_ini_wib("a.verified_at"))`.
pub(crate) fn hari_ini_wib(kolom: &str) -> String {
    const AWAL: &str =
        "(date_trunc('day', NOW() AT TIME ZONE 'Asia/Jakarta') AT TIME ZONE 'Asia/Jakarta')";
    format!("{kolom} >= {AWAL} AND {kolom} < {AWAL} + INTERVAL '1 day'")
}

pub(crate) fn kelas_utama_lateral(user_col: &str) -> String {
    format!(
        "LEFT JOIN LATERAL ( \
            SELECT c.id, c.name, c.category, c.jenjang, c.description, \
                   c.wali_kelas_id \
              FROM class_participants cp_ku \
              JOIN classes c ON c.id = cp_ku.class_id \
             WHERE cp_ku.user_id = {user_col} \
             ORDER BY (c.category = 'kbm') DESC, c.id \
             LIMIT 1 \
        ) cl ON TRUE"
    )
}
