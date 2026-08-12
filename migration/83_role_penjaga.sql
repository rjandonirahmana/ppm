-- =============================================================================
-- 83_role_penjaga.sql — Peran baru: PENJAGA.
--
-- Tugasnya satu dan jelas: mengecek ulang apakah tamu benar-benar mengisi data
-- dirinya dengan benar. Buku tamu sudah berjalan sejak migrasi 35 — tamu mengisi
-- /tamu, dapat kode 6 digit, lalu mengetiknya di mesin RFID yang memotret
-- wajahnya — tapi baris `guest_visits` yang lahir dari situ SELAMA INI TAK
-- PERNAH DIBACA siapa pun. Datanya terkumpul rapi dan tak ada yang melihatnya.
--
-- Penjaga adalah orang yang melihatnya: mencocokkan wajah yang terpotret dengan
-- nama dan keperluan yang diketik, dan menandai yang tak cocok.
--
-- KENAPA PERAN SENDIRI, bukan menumpang admin. Penjaga gerbang tak perlu — dan
-- tak boleh — melihat poin santri, tagihan, atau perizinan. Memberinya peran
-- admin agar bisa membuka satu layar berarti membuka seluruh isi aplikasi
-- kepada pos jaga.
--
-- Idempotent. Jalankan setelah migrasi 1–82.
-- =============================================================================

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN (
        'admin', 'ketua', 'dewan_guru', 'supervisor',
        'santri', 'santri_finance', 'parent', 'penjaga'
    ));

-- ── Penandaan hasil pemeriksaan penjaga ──────────────────────────────────────
-- NULL = belum diperiksa. Dipisah dari `checked_in_at`: yang itu dicatat mesin
-- saat wajah terpotret, yang ini dicatat manusia saat mencocokkannya.
ALTER TABLE guest_visits ADD COLUMN IF NOT EXISTS verified_by BIGINT
    REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE guest_visits ADD COLUMN IF NOT EXISTS verified_at TIMESTAMPTZ;
-- Catatan penjaga bila datanya JANGGAL — kosong/NULL saat data dinyatakan cocok.
ALTER TABLE guest_visits ADD COLUMN IF NOT EXISTS verify_note TEXT;

-- Antrean "belum diperiksa", terbaru dulu. Parsial: begitu diperiksa, barisnya
-- keluar dari antrean dan tak perlu lagi menempati index.
CREATE INDEX IF NOT EXISTS idx_guest_visits_belum_diperiksa
    ON guest_visits (checked_in_at DESC) WHERE verified_at IS NULL;

ANALYZE users;
ANALYZE guest_visits;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   SELECT pg_get_constraintdef(oid) FROM pg_constraint
--    WHERE conrelid = 'users'::regclass AND conname = 'users_role_check';
--   -- harus memuat 'penjaga'.
--
--   SELECT column_name FROM information_schema.columns
--    WHERE table_name = 'guest_visits'
--      AND column_name IN ('verified_by','verified_at','verify_note');
--   -- harus 3 baris.
--
--   -- Berapa kunjungan yang menunggu diperiksa:
--   SELECT count(*) FROM guest_visits WHERE verified_at IS NULL;
--
--   -- Membuat akun penjaga pertama (ganti id-nya):
--   -- UPDATE users SET role = 'penjaga' WHERE id = ...;
-- =============================================================================
