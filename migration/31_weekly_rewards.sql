-- =============================================================================
-- 31_weekly_rewards.sql — Reward poin mingguan (PRD "Sistem Poin 2.0" hal. 8).
--
-- Reward per KATEGORI kegiatan per pekan (Senin–Minggu):
--   No-Alfa   : KBM +5, Non-KBM +3, Piket +2   (tak ada alfa & santri hadir)
--   No-Telat  : KBM +13, Non-KBM +8            (tak ada telat & hadir)
--   Full-Hadir: KBM +20, Non-KBM +12           (hadir sempurna: tanpa alfa/telat/
--                                                izin/sakit)
-- Santri bisa dapat SEMUA reward bila memenuhi semua kualifikasi. Dikreditkan
-- pengurus (admin) tiap Senin utk pekan sebelumnya.
--
-- weekly_rewards = penjamin SEKALI kredit per (santri, pekan) — UNIQUE. Kredit
-- juga menulis point_logs (kategori 'achievement') + menaikkan users.points.
--
-- Idempotent. Setelah migrasi 1–30.
-- =============================================================================

CREATE TABLE IF NOT EXISTS weekly_rewards (
    id         BIGSERIAL PRIMARY KEY,
    user_id    BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    week_start DATE   NOT NULL,               -- Senin pekan bersangkutan (WIB)
    points     INT    NOT NULL,
    detail     TEXT,                          -- rincian, mis. "KBM Full Hadir +20; ..."
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, week_start)
);
CREATE INDEX IF NOT EXISTS idx_weekly_rewards_week ON weekly_rewards (week_start);
