//! repository — Query Postgres (server-only), dipisah per-domain.
//! Baris → struct sederhana; pemformatan tampilan dilakukan di layer service.

pub mod activity_log;
pub mod activity_photos;
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
pub mod settings;
pub mod users;

pub use activity_log::*;
pub use activity_photos::*;
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
pub use settings::*;
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
pub(crate) fn kelas_utama_lateral(user_col: &str) -> String {
    format!(
        "LEFT JOIN LATERAL ( \
            SELECT c.id, c.name, c.category, c.jenjang, c.description, \
                   c.wali_kelas_id, c.pamong_id, c.require_pamong, c.verify_mode \
              FROM class_participants cp_ku \
              JOIN classes c ON c.id = cp_ku.class_id \
             WHERE cp_ku.user_id = {user_col} \
             ORDER BY (c.category = 'kbm') DESC, c.id \
             LIMIT 1 \
        ) cl ON TRUE"
    )
}
