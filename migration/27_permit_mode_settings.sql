-- =============================================================================
-- 27_permit_mode_settings.sql — Setelan global aplikasi + tahap GURU pada izin.
--
-- Fitur: admin bisa memilih mode persetujuan izin (global):
--   * 'two_stage'   → Pamong menyetujui DULU, lalu Guru (2 persetujuan).
--   * 'direct_guru' → langsung ke Guru (1 persetujuan, pamong dilewati).
--
-- Skema izin BARU: parent_status → pamong_status (opsional, hanya two_stage) →
-- guru_status (KEPUTUSAN FINAL di KEDUA mode). "Izin sah" = guru_status='approved'.
--
-- Data lama (single-stage, keputusan efektif = pamong) DIPINDAH: guru_status
-- mewarisi pamong_status agar izin yang sudah disetujui/ditolak tetap final.
-- Idempotent. Setelah migrasi 1–26.
-- =============================================================================

CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

INSERT INTO app_settings (key, value) VALUES ('permit_approval_mode', 'two_stage')
    ON CONFLICT (key) DO NOTHING;

ALTER TABLE permit_requests
    ADD COLUMN IF NOT EXISTS guru_status VARCHAR(10) NOT NULL DEFAULT 'pending';
ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS guru_by BIGINT;
ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS guru_at TIMESTAMPTZ;

-- Warisi keputusan lama (single-stage) ke tahap guru — sekali saja untuk baris
-- yang pamong-nya sudah memutuskan sebelum kolom guru_status ada.
UPDATE permit_requests
   SET guru_status = pamong_status, guru_at = pamong_at, guru_by = pamong_by
 WHERE pamong_status <> 'pending' AND guru_status = 'pending';
