-- =============================================================================
-- 40_academic_semesters.sql — Semester akademik yang DIDEFINISIKAN admin
-- (ganjil/genap, tahun, rentang tanggal). Menggantikan asumsi otomatis
-- (Jul–Des=Ganjil, Jan–Jun=Genap) bila ada baris yang di-set aktif: dipakai
-- `service::santri::current_semester` sebagai acuan awal semester (kehadiran %,
-- laporan). Hanya SATU semester boleh aktif (partial unique index).
-- Idempotent. Setelah migrasi 1–39.
-- =============================================================================

CREATE TABLE IF NOT EXISTS academic_semesters (
    id          BIGSERIAL PRIMARY KEY,
    kind        TEXT     NOT NULL CHECK (kind IN ('ganjil', 'genap')),
    year        SMALLINT NOT NULL,                 -- tahun awal (2026 → "2026/2027")
    start_date  DATE     NOT NULL,
    end_date    DATE     NOT NULL,
    is_active   BOOLEAN  NOT NULL DEFAULT FALSE,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Maksimal satu semester aktif pada satu waktu.
CREATE UNIQUE INDEX IF NOT EXISTS idx_semester_one_active
    ON academic_semesters (is_active) WHERE is_active;
