-- =============================================================================
-- 11_hafalan.sql — KERANGKA laporan akademik kategori "Mengaji"/"Pengajian":
-- log setoran hafalan santri (surah/ayat/juz/kualitas), dicatat staf.
--
-- Append-only (pola sama point_logs) — agregasi (juz selesai, ranking, riwayat)
-- dihitung on-demand dari log ini, bukan kolom cache. class_id opsional (setoran
-- bisa dicatat lepas dari sesi tertentu); recorded_by = staf pencatat.
--
-- Idempotent. Jalankan setelah migrasi 1–10.
-- =============================================================================

CREATE TABLE IF NOT EXISTS hafalan_logs (
    id           BIGSERIAL PRIMARY KEY,

    user_id      BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    class_id     BIGINT REFERENCES classes(id) ON DELETE SET NULL,
    recorded_by  BIGINT REFERENCES users(id) ON DELETE SET NULL,

    surah        VARCHAR(50) NOT NULL,
    ayat_range   VARCHAR(20) NOT NULL DEFAULT '',
    juz          SMALLINT,

    quality      VARCHAR(20) NOT NULL DEFAULT 'lancar' CHECK (
                    quality IN ('lancar', 'perlu_perbaikan', 'mengulang')
                 ),
    note         TEXT,

    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_hafalan_user ON hafalan_logs (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_hafalan_juz ON hafalan_logs (user_id, juz) WHERE juz IS NOT NULL;
