-- =============================================================================
-- 29_class_wali_kelas.sql — Wali kelas + rute persetujuan izin PER-KELAS.
--
-- Tiap kelas (Cepatan/Lambatan/Saringan/Hadist Besar, dll) punya:
--   • wali_kelas_id  → guru penyetuju FINAL izin santri kelas itu.
--   • require_pamong → TRUE: izin lewat Pamong dulu, baru Wali Kelas (2 tahap);
--                      FALSE: langsung ke Wali Kelas.
--
-- Rute izin santri ditentukan dari KELAS UTAMA santri (class_participants.
-- is_primary). Mengganti mode global 'permit_approval_mode' (migrasi 27) yang
-- kini hanya jadi fallback untuk santri TANPA kelas utama.
--
-- Idempotent. Setelah migrasi 1–28.
-- =============================================================================

ALTER TABLE classes
    ADD COLUMN IF NOT EXISTS wali_kelas_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE classes
    ADD COLUMN IF NOT EXISTS require_pamong BOOLEAN NOT NULL DEFAULT TRUE;

CREATE INDEX IF NOT EXISTS idx_classes_wali ON classes (wali_kelas_id)
    WHERE wali_kelas_id IS NOT NULL;
