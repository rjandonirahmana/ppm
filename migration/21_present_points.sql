-- =============================================================================
-- 21_present_points.sql — Poin kehadiran DISERAGAMKAN jadi MAGNITUDO POSITIF.
--
-- Sebelumnya campur aduk & membingungkan:
--   • late_points (migrasi 13) = delta BERTANDA langsung (mis. -5).
--   • absent_points (migrasi 15) = magnitude positif, dikurangkan.
-- Kini SEMUA poin positif; ARAH operasi ditentukan kode (models::attendance_delta):
--   • present_points (BARU) → DITAMBAH saat tepat waktu (default 10).
--   • late_points          → DIKURANGI saat telat (default 0).
--   • absent_points        → DIKURANGI saat alpa (default 15).
-- Tak ada lagi nilai minus di DB/UI → hilangkan kebingungan.
--
-- Idempotent. Jalankan setelah migrasi 1–20.
-- =============================================================================

ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS present_points SMALLINT;

-- Konversi nilai lama late_points (dulu bisa negatif = penalti) → magnitudo
-- positif, karena semantik kini "dikurangi". |−5| = 5 (efek penalti sama).
UPDATE class_schedules SET late_points = ABS(late_points) WHERE late_points IS NOT NULL;

-- Batas wajar (opsional; app juga memvalidasi 0..=100).
ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS chk_present_points;
ALTER TABLE class_schedules ADD CONSTRAINT chk_present_points
    CHECK (present_points IS NULL OR present_points BETWEEN 0 AND 100);
