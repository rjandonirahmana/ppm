-- =============================================================================
-- 33_session_pamong.sql — Pamong PENUGAS per-SESI (verifikasi kehadiran).
--
-- Pamong KELAS (classes.pamong_id, migrasi 30) berperan sebagai PENUGAS: ~1 jam
-- sebelum sesi ia mengedit (di /sesi/:id) SIAPA ustad pengajar (class_sessions.
-- teacher_id, sudah ada) DAN SIAPA pamong yang bertugas memverifikasi kehadiran
-- di sesi itu (class_sessions.pamong_id, BARU) — karena bisa jadi pamong kelas
-- sedang di luar lokasi dan menugaskan orang lain.
--
-- Verifikasi kehadiran kini PER-SESI:
--   tahap 1 (pamong_status)  = pamong bertugas sesi (COALESCE cs.pamong_id, cl.pamong_id)
--   tahap 2 (verify_status)  = ustad bertugas sesi (COALESCE cs.teacher_id, cl.wali_kelas_id)
--   2 langkah bila classes.require_pamong; 1 langkah (hanya ustad) bila tidak.
--
-- Idempotent. Setelah migrasi 1–32.
-- =============================================================================

ALTER TABLE class_sessions
    ADD COLUMN IF NOT EXISTS pamong_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_sessions_pamong ON class_sessions (pamong_id)
    WHERE pamong_id IS NOT NULL;
