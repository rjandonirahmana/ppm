-- =============================================================================
-- 42_scan_date_unique.sql — Fix attendance duplikat (critical security)
--
-- Tambahkan kolom scan_date (tanggal WIB) untuk UNIQUE constraint yang benar.
-- Masalah: race condition antara attendance_exists_today & insert_attendance.
-- Solusi: UNIQUE(user_id, COALESCE(class_session_id,-1), scan_date) + idempoten.
--
-- Idempotent. Jalankan setelah migrasi 1–41.
-- =============================================================================

-- 1) Tambahkan kolom scan_date (optional awalnya, untuk backfill safe).
ALTER TABLE attendances
    ADD COLUMN IF NOT EXISTS scan_date DATE;

-- 2) Backfill scan_date dari scanned_at (WIB timezone).
-- Timezone Jakarta (UTC+7): SET timezone = 'Asia/Jakarta'
UPDATE attendances
SET scan_date = (scanned_at AT TIME ZONE 'Asia/Jakarta')::date
WHERE scan_date IS NULL;

-- 3) Buat constraint UNIQUE untuk prevent duplikat per hari.
-- COALESCE(class_session_id, -1) untuk handle NULL sessions.
CREATE UNIQUE INDEX IF NOT EXISTS uq_attendance_daily
    ON attendances (user_id, COALESCE(class_session_id, -1), scan_date);

-- 4) Validasi minimal (optional, untuk audit).
-- Jika ada data NULL setelah backfill, ada celah data yang perlu ditangani.
-- SELECT COUNT(*) FROM attendances WHERE scan_date IS NULL;
