-- =============================================================================
-- 5_parent_connections.sql — Koneksi orang tua ↔ santri (BANYAK anak + approval).
--
-- Kebutuhan baru: (1) satu orang tua bisa terhubung ke BEBERAPA santri;
-- (2) koneksi butuh PERSETUJUAN SANTRI (parent kirim permintaan → santri
-- setujui/tolak). Kolom tunggal users.related_id tidak bisa menampung
-- keduanya → tabel relasi ini menggantikannya (data lama dimigrasikan).
--
-- Idempotent. Jalankan setelah migrasi 1–4:
--   psql "$DATABASE_URL" -f migration/5_parent_connections.sql
-- =============================================================================

CREATE TABLE IF NOT EXISTS parent_connections (
    id              BIGSERIAL PRIMARY KEY,

    parent_id       BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    student_id      BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,

    -- pending  : menunggu persetujuan SANTRI
    -- connected: disetujui santri → ortu bisa memantau
    -- rejected : ditolak santri
    status          VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (
                        status IN ('pending','connected','rejected')
                    ),

    requested_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    responded_at    TIMESTAMPTZ,

    UNIQUE (parent_id, student_id)
);
CREATE INDEX IF NOT EXISTS idx_pconn_parent  ON parent_connections (parent_id);
CREATE INDEX IF NOT EXISTS idx_pconn_student ON parent_connections (student_id) WHERE status = 'pending';

-- Migrasikan relasi lama users.related_id → connected, lalu hapus kolomnya.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
               WHERE table_name = 'users' AND column_name = 'related_id') THEN
        INSERT INTO parent_connections (parent_id, student_id, status, responded_at)
        SELECT id, related_id, 'connected', NOW()
        FROM   users
        WHERE  role = 'parent' AND related_id IS NOT NULL
        ON CONFLICT (parent_id, student_id) DO NOTHING;

        DROP INDEX IF EXISTS idx_users_related;
        ALTER TABLE users DROP COLUMN related_id;
    END IF;
END $$;
