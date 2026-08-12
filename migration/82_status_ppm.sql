-- =============================================================================
-- 82_status_ppm.sql — Status keanggotaan santri di PPM.
--
-- Sampai sekarang satu-satunya penanda "masih di sini atau tidak" adalah
-- `users.is_active` — sebuah boolean. Ia menjawab "boleh masuk aplikasi?" tapi
-- tak menjawab "kenapa ia tidak lagi di sini", padahal jawabannya berbeda-beda
-- dan penting: santri yang LULUS adalah alumni yang pantas dirayakan, yang
-- MENGUNDURKAN DIRI berhenti atas kehendak sendiri, dan yang PINDAH melanjutkan
-- di tempat lain. Ketiganya kini sama-sama tampil sebagai "nonaktif" saja.
--
-- Dipisah dari `is_active`, bukan menggantikannya: yang satu soal AKSES, yang
-- ini soal RIWAYAT. Alumni bisa saja tetap boleh masuk untuk melihat rapornya,
-- dan santri yang aksesnya dicabut sementara karena pelanggaran bukan berarti
-- statusnya berubah.
--
-- NULL = masih santri aktif, dan itu keadaan bawaan seluruh baris yang sudah
-- ada. Tak ada data yang perlu ditebak-tebak saat migrasi ini jalan.
--
-- CHECK mengizinkan NULL SECARA EKSPLISIT. Tanpa `IS NULL`, baris tak terisi
-- sebenarnya tetap lolos (CHECK bernilai NULL dianggap lulus) — hasilnya benar,
-- tapi hanya secara kebetulan. Menuliskannya membuat niatnya terbaca (pola yang
-- sama dengan migrasi 73).
--
-- Idempotent. Jalankan setelah migrasi 1–81.
-- =============================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS status_ppm VARCHAR(20);

ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_status_ppm;
ALTER TABLE users ADD CONSTRAINT chk_users_status_ppm
    CHECK (status_ppm IS NULL OR status_ppm IN (
        'aktif', 'lulus', 'mengundurkan_diri', 'pindah'
    ));

COMMENT ON COLUMN users.status_ppm IS
    'Status keanggotaan di PPM. NULL/aktif = masih santri. Terpisah dari '
    'is_active, yang mengatur AKSES, bukan riwayat.';

-- Dicari saat menyusun daftar alumni & rekap keluar-masuk santri. Parsial:
-- sebagian besar baris NULL (masih aktif), dan index atasnya hanya membebani
-- tulis tanpa pernah terpakai.
CREATE INDEX IF NOT EXISTS idx_users_status_ppm
    ON users (status_ppm) WHERE status_ppm IS NOT NULL;

ANALYZE users;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   SELECT column_name, data_type, is_nullable FROM information_schema.columns
--    WHERE table_name = 'users' AND column_name = 'status_ppm';
--
--   SELECT pg_get_constraintdef(oid) FROM pg_constraint
--    WHERE conrelid = 'users'::regclass AND conname = 'chk_users_status_ppm';
--
--   -- Sebaran (setelah migrasi: semua NULL):
--   SELECT COALESCE(status_ppm,'(aktif)') AS status, count(*)
--     FROM users WHERE role IN ('santri','santri_finance') GROUP BY 1 ORDER BY 2 DESC;
--
--   -- Nilai ngawur HARUS ditolak:
--   -- UPDATE users SET status_ppm = 'keluar' WHERE id = 1;
-- =============================================================================
