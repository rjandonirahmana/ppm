-- =============================================================================
-- 4_parent_related.sql — Gabungkan relasi orang tua ke tabel users.
--
-- Keputusan: TIDAK ada tabel parent_students terpisah. Orang tua = users dengan
-- role='parent' dan `related_id` = user_id SANTRI yang dipantau.
--
-- Migrasi ini untuk DB yang terlanjur menjalankan 2.sql/3.sql versi lama
-- (yang masih membuat parent_students). Idempotent — aman dijalankan berulang,
-- juga aman di DB baru (tabelnya memang tidak ada).
--   psql "$DATABASE_URL" -f migration/4_parent_related.sql
-- =============================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS related_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_users_related ON users (related_id);

-- Pindahkan data lama (bila tabel parent_students ada) lalu hapus tabelnya.
DO $$
BEGIN
    IF to_regclass('public.parent_students') IS NOT NULL THEN
        UPDATE users u
        SET    related_id = ps.student_id
        FROM   parent_students ps
        WHERE  ps.parent_id = u.id
          AND  u.related_id IS NULL;

        DROP TABLE parent_students;
    END IF;
END $$;
