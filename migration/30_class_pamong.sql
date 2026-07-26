-- =============================================================================
-- 30_class_pamong.sql — Pamong PER-KELAS + penanda pengingat WA sesi.
--
-- Tiap kelas punya PAMONG sendiri (selain wali kelas, migrasi 29). Tugas pamong
-- kelas:
--   1. Verifikasi kehadiran (gate) santri kelas itu (attendances tahap pamong).
--   2. Tahap-1 persetujuan izin santri kelas itu (bila require_pamong).
--   3. Menerima WA ~1 jam sebelum sesi untuk mengatur dewan guru pengisi.
--
-- class_sessions.pamong_notified_at = penanda idempotent agar WA pengingat sesi
-- hanya dikirim SEKALI per sesi.
--
-- Idempotent. Setelah migrasi 1–29.
-- =============================================================================

ALTER TABLE classes
    ADD COLUMN IF NOT EXISTS pamong_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_classes_pamong ON classes (pamong_id)
    WHERE pamong_id IS NOT NULL;

ALTER TABLE class_sessions ADD COLUMN IF NOT EXISTS pamong_notified_at TIMESTAMPTZ;
