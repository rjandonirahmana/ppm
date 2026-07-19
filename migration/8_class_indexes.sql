-- =============================================================================
-- 8_class_indexes.sql — index yang hilang untuk halaman kelas & materialisasi sesi.
--
-- Semua query di /kelas & /kelas/:id memfilter per class_id / (schedule_id, date),
-- tapi index-nya belum ada → seq-scan (cepat saat data sedikit, LAMBAT saat besar).
--   • class_members            : WHERE cp.class_id = $1
--   • sessions_of_class        : WHERE s.class_id = $1 ORDER BY session_date DESC
--   • insert_sessions NOT EXISTS: (class_schedule_id, session_date)
--
-- Additive & idempotent. Jalankan setelah migrasi 1–7.
--   psql "$DATABASE_URL" -f migration/8_class_indexes.sql
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_cp_class
    ON class_participants (class_id);

CREATE INDEX IF NOT EXISTS idx_cs_class_date
    ON class_sessions (class_id, session_date DESC);

CREATE INDEX IF NOT EXISTS idx_cs_schedule_date
    ON class_sessions (class_schedule_id, session_date);
