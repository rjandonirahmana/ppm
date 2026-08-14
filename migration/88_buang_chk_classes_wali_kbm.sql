-- =============================================================================
-- 88_buang_chk_classes_wali_kbm.sql — Buang CHECK "wali hanya boleh di kelas
-- KBM" yang tertinggal hidup.
--
-- ── KENAPA MASIH ADA PADAHAL MIGRASI 84 MEMBUANGNYA ──────────────────────────
-- Migrasi 84 memang memuat `DROP CONSTRAINT IF EXISTS chk_classes_wali_kbm`,
-- tapi baris itu ada di BAGIAN 3 — sesudah bagian 2. Dan bagian 2 versi
-- pertamanya menjalankan:
--
--     UPDATE classes SET verify_mode = 'guru' WHERE verify_mode <> 'guru';
--
-- yang DITOLAK `chk_classes_verify_non_kbm` (migrasi 67: "kelas non-KBM wajib
-- verify_mode = 'pamong'"). Migrasi 84 karena itu berhenti di tengah pada
-- database yang menjalankannya sebelum urutan itu diperbaiki, dan semua yang
-- ada SESUDAH titik gagal tak pernah jalan — termasuk pembuangan CHECK ini.
--
-- Migrasi 84 kini sudah membuang pagar itu lebih dulu, jadi menjalankannya ulang
-- juga menyelesaikan masalah ini. Berkas terpisah tetap dibuat karena
-- `scripts/migrate.sh` mencatat migrasi yang sudah dijalankan dan tak akan
-- mengulang 84 dengan sendirinya — sementara database yang terlanjur setengah
-- jalan tetap harus diperbaiki tanpa campur tangan tangan.
--
-- ── AKIBATNYA HARI INI ───────────────────────────────────────────────────────
-- Setiap upaya menetapkan wali kelas untuk kelas NON-KBM ditolak database:
--
--     ERROR: new row for relation "classes" violates check constraint
--            "chk_classes_wali_kbm"
--
-- Padahal sejak peran pamong dihapus, wali kelas adalah satu-satunya jabatan
-- yang tersisa — dan setiap kelas wajib punya, apa pun kategorinya.
--
-- ⚠️ YANG TAK BISA DIPULIHKAN OLEH MIGRASI INI. Bagian 3 migrasi 84 juga berisi
-- pengangkatan `pamong_id` menjadi `wali_kelas_id` untuk kelas yang belum punya
-- wali. Langkah itu ikut tak pernah jalan, dan `classes.pamong_id` sudah dibuang
-- migrasi 86 — datanya tak ada lagi. Kelas non-KBM yang dulu diurus pamong
-- karena itu HARUS ditetapkan walinya kembali secara manual lewat
-- "Wali Kelas & Verifikasi" di detail kelas. Daftar kelas menandainya dengan
-- peringatan kuning; sesudah migrasi ini, penetapannya tak lagi ditolak.
--
-- Idempotent. Jalankan setelah migrasi 1–87.
-- =============================================================================

ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_wali_kbm;

-- Sekalian pagar sezamannya, kalau-kalau ikut tertinggal karena sebab yang sama.
-- Keduanya menyebut kolom yang sudah dibuang migrasi 86; `IF EXISTS` membuatnya
-- tak melakukan apa-apa bila memang sudah bersih.
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_non_kbm;
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_mode;

ANALYZE classes;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   -- Ketiga pagar lama sudah hilang (harus 0 baris):
--   SELECT conname FROM pg_constraint
--    WHERE conrelid = 'classes'::regclass
--      AND conname IN ('chk_classes_wali_kbm', 'chk_classes_verify_non_kbm',
--                      'chk_classes_verify_mode');
--
--   -- Semua CHECK yang TERSISA di classes — periksa tak ada lagi yang menyebut
--   -- kolom/peran yang sudah tak ada:
--   SELECT conname, pg_get_constraintdef(oid) FROM pg_constraint
--    WHERE conrelid = 'classes'::regclass AND contype = 'c';
--
--   -- Kelas yang masih butuh wali kelas ditetapkan manual:
--   SELECT id, name, category FROM classes
--    WHERE wali_kelas_id IS NULL AND status = 'active' ORDER BY category, name;
-- =============================================================================
