-- =============================================================================
-- 23_schedule_custom_dates.sql — Jadwal "Tanggal tertentu" (recurrence_type
-- 'custom'): daftar tanggal MANUAL yang loncat-loncat (bukan pola harian/
-- mingguan/bulanan). Disimpan JSONB array "YYYY-MM-DD".
--
-- Sesi dimaterialisasi LANGSUNG dari daftar ini saat buat/ubah jadwal (bukan
-- lewat pola). start_date/end_date jadwal di-set otomatis = min/max tanggal.
-- Idempotent. Jalankan setelah migrasi 1–22.
-- =============================================================================

ALTER TABLE class_schedules
    ADD COLUMN IF NOT EXISTS custom_dates JSONB NOT NULL DEFAULT '[]'::jsonb;
