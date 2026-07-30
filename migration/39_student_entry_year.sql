-- =============================================================================
-- 39_student_entry_year.sql — Tahun masuk kuliah santri (mahasiswa).
-- Melengkapi profil mahasiswa (migrasi 26: campus/major/gender + ipk_history).
-- Diisi SENDIRI oleh santri di halaman profil. SMALLINT cukup (tahun 4 digit).
-- Idempotent. Setelah migrasi 1–38.
-- =============================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS entry_year SMALLINT;  -- mis. 2023
