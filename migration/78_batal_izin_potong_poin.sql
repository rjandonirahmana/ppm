-- =============================================================================
-- 78_batal_izin_potong_poin.sql — Buang `classes.izin_potong_poin` bila sempat
-- terlanjur dibuat.
--
-- ── APA YANG TERJADI ─────────────────────────────────────────────────────────
-- Sempat ada berkas `77_izin_potong_poin_per_kelas.sql` yang menambah kolom
-- boolean `classes.izin_potong_poin` sebagai setelan "izin memotong poin?".
-- Kolom itu KELIRU sejak awal: jawabannya sudah punya rumah sendiri di
-- `class_schedules.izin_points` (migrasi 28) — kolom yang memang dibuat untuk
-- ini, sudah bisa disunting dari form jadwal, dan letaknya lebih tepat karena
-- kebijakannya berbeda antar KEGIATAN, bukan antar kelas. Dalam satu kelas yang
-- sama, ngaji bisa wajib hadir sementara kajian tambahan tidak.
--
-- Berkas 77 itu sudah dihapus dari repo sebelum sempat menjadi bagian rilis.
-- Migrasi ini ada untuk satu kemungkinan saja: berkas itu terlanjur dijalankan
-- di sebuah database sebelum dicabut.
--
-- ── KENAPA NOMOR 78, BUKAN 77 ────────────────────────────────────────────────
-- `scripts/migrate.sh` mencatat migrasi berdasarkan NOMOR VERSI. Kalau 77
-- terlanjur tercatat di suatu database, migrasi bernomor 77 apa pun sesudahnya
-- akan dilewati diam-diam — persis kelas kegagalan yang skrip itu dibuat untuk
-- mencegah (lihat catatan tentang migrasi 42 di sana). Nomor 77 karena itu
-- DIPENSIUNKAN dan dibiarkan berlubang; jangan dipakai ulang.
--
-- ── AMAN DIJALANKAN DI MANA PUN ──────────────────────────────────────────────
-- `IF EXISTS` membuatnya tak melakukan apa-apa pada database yang tak pernah
-- menjalankan berkas 77 — yaitu keadaan yang diharapkan. Tak ada kode yang
-- membaca kolom ini (`grep izin_potong_poin src/` = kosong), jadi membuangnya
-- tak memutus apa pun.
--
-- Idempotent. Jalankan setelah migrasi 1–76.
-- =============================================================================

ALTER TABLE classes DROP COLUMN IF EXISTS izin_potong_poin;

ANALYZE classes;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   SELECT column_name FROM information_schema.columns
--    WHERE table_name = 'classes' AND column_name = 'izin_potong_poin';
--   -- HARUS kosong.
--
--   -- Apakah database ini sempat menjalankan berkas 77 yang dicabut itu?
--   SELECT version, name, applied_at FROM schema_migrations WHERE version = 77;
--   -- Ada baris  → berkas 77 pernah jalan; migrasi ini yang membereskannya.
--   -- Kosong     → memang tak pernah jalan; migrasi ini tak mengubah apa pun.
-- =============================================================================
