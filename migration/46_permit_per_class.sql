-- =============================================================================
-- 46_permit_per_class.sql — Permit workflow refactor: per-wali kelas, hapus parent approval
--
-- Perubahan:
-- 1) Hapus parent_status & related columns (tidak perlu orang tua approve izin akademik)
-- 2) Tambah class_id & wali_kelas_id untuk track kelas yang affected
-- 3) Tambah columns untuk multi-class approval tracking
--
-- Logika baru:
-- - Santri izin 2 hari, ada 3 kelas berbeda → auto-create 3 permit requests
--   (group by wali_kelas_id unik yang affected)
-- - Setiap permit_request perlu approval dari:
--   a) Pamong kelas (jika require_pamong)
--   b) Wali kelas bersangkutan (final)
--
-- Idempotent. Jalankan setelah migrasi 1–45.
-- =============================================================================

-- ═ 1) Hapus parent approval columns ═════════════════════════════════════════
ALTER TABLE permit_requests
    DROP COLUMN IF EXISTS parent_status,
    DROP COLUMN IF EXISTS parent_confirmed_by,
    DROP COLUMN IF EXISTS parent_confirmed_at;

-- ═ 2) Tambah class_id & wali_kelas_id untuk link ke kelas specific ════════
-- (kelas apa yang affected oleh izin ini, dan siapa wali kelasnya)
ALTER TABLE permit_requests
    ADD COLUMN IF NOT EXISTS class_id BIGINT REFERENCES classes(id) ON DELETE CASCADE,
    ADD COLUMN IF NOT EXISTS wali_kelas_id BIGINT REFERENCES users(id) ON DELETE SET NULL;

-- ═ 3) Rename pamong_status → pamong_status (keep sama) & tambah guru_status ═══
-- (sudah ada guru_status dari migration sebelumnya, confirm disini saja)
ALTER TABLE permit_requests
    ADD COLUMN IF NOT EXISTS guru_status VARCHAR(20) DEFAULT 'pending',
    ADD CONSTRAINT permit_requests_guru_status_check
        CHECK (guru_status IN ('pending', 'approved', 'rejected'));

-- ═ 4) Track approval dates ═════════════════════════════════════════════════
ALTER TABLE permit_requests
    ADD COLUMN IF NOT EXISTS guru_by BIGINT REFERENCES users(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS guru_at TIMESTAMPTZ;

-- ═ 5) Index untuk efficient lookup: antrean approval per guru ═══════════════
CREATE INDEX IF NOT EXISTS idx_permit_guru_pending
    ON permit_requests (wali_kelas_id, guru_status)
    WHERE guru_status = 'pending';

CREATE INDEX IF NOT EXISTS idx_permit_pamong_pending
    ON permit_requests (class_id, pamong_status)
    WHERE pamong_status = 'pending';

-- ═ 6) Komposit index: permit yang punya class + guru unik ═══════════════════
CREATE UNIQUE INDEX IF NOT EXISTS uq_permit_class_wali
    ON permit_requests (user_id, start_date, COALESCE(end_date, start_date), class_id, wali_kelas_id)
    WHERE guru_status <> 'rejected';
    -- Cegah duplikasi: santri tidak bisa ajukan izin sama untuk class+tanggal yang sama

-- ═ 7) Update existing permits: backfill class_id & wali_kelas_id ═════════════
-- (dari kelas utama santri - is_primary)
UPDATE permit_requests pr SET
    class_id = (
        SELECT cp.class_id FROM class_participants cp
        WHERE cp.user_id = pr.user_id AND cp.is_primary LIMIT 1
    ),
    wali_kelas_id = (
        SELECT c.wali_kelas_id FROM class_participants cp
        JOIN classes c ON c.id = cp.class_id
        WHERE cp.user_id = pr.user_id AND cp.is_primary LIMIT 1
    )
WHERE class_id IS NULL;

-- ═ 8) Query validation (run di staging sebelum production) ═══════════════════
-- Cek berapa banyak permit yang ter-backfill dengan class_id NULL
-- SELECT COUNT(*) FROM permit_requests WHERE class_id IS NULL;
-- Jika > 0, ada santri tanpa kelas utama - handle manually atau default ke supervisor.
