-- =============================================================================
-- 26_santri_profile_ipk.sql — Kolom profil santri (mahasiswa) + riwayat IPK.
-- Santri PPM = mahasiswa kampus sekitar (UI/Gunadarma/PNJ), jadi punya kampus,
-- jurusan, gender, dan IPK per-semester (riwayat). Diisi SENDIRI oleh santri di
-- halaman profil.
--
-- ipk DOUBLE PRECISION (0..4) — cukup untuk tampilan 2 desimal; tokio-postgres
-- dukung f64 native (hindari dependency rust_decimal untuk NUMERIC).
-- Idempotent. Setelah migrasi 1–25.
-- =============================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS campus VARCHAR(150);
ALTER TABLE users ADD COLUMN IF NOT EXISTS major  VARCHAR(150);
ALTER TABLE users ADD COLUMN IF NOT EXISTS gender VARCHAR(10);   -- 'L' | 'P'

CREATE TABLE IF NOT EXISTS ipk_history (
    id         BIGSERIAL PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    semester   VARCHAR(40) NOT NULL,               -- mis. "2024/2025 Ganjil"
    ipk        DOUBLE PRECISION NOT NULL CHECK (ipk >= 0 AND ipk <= 4),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_ipk_history_user ON ipk_history (user_id, id);
