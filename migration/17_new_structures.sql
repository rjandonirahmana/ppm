-- =============================================================================
-- 17_new_structures.sql — Enam kesenjangan nyata ditemukan saat membandingkan
-- skema `ppm` (16 migrasi) dengan skema baru yang disodorkan user (konsolidasi
-- "ppm-new"), setelah dikonfirmasi mana yang SUDAH ada (golongan/category dll,
-- migrasi 6/10/16) vs mana yang benar-benar baru:
--
--   1) users.role CHECK tak pernah memasukkan 'dewan_guru' — padahal peran ini
--      dipakai luas di kode (routing/nav/otorisasi) sejak lama. Celah migrasi,
--      diperbaiki di sini.
--   2) class_schedules.room — BARU. Kolom "Room" di Weekly Class Schedule.
--   3) permit_requests DUA TAHAP (Orang Tua → Pamong) — BARU. Skema lama cuma
--      1 tahap (status/reviewed_by/reviewed_at/review_note) dan TAK PERNAH
--      dipakai aplikasi (grep menyeluruh: hanya insert_permit/list_my_permits
--      ada, tak ada fungsi approve/reject sama sekali — status selalu
--      'pending' selamanya). Karena kolom lama 100% mati di kode, di-RENAME
--      (bukan ditambah di samping) supaya konsisten dgn pola dua-tahap
--      attendances (pamong_status/verify_status) yang sudah ada:
--        status → pamong_status, reviewed_by → pamong_by,
--        reviewed_at → pamong_at, review_note → pamong_note.
--      + kolom BARU requested_by/parent_status/parent_confirmed_by/at.
--      Permohonan diajukan SANTRI sendiri → parent_status='pending' (perlu
--      dikonfirmasi orang tua terhubung lebih dulu); diajukan LANGSUNG oleh
--      orang tua → 'approved' otomatis (parent adalah pengaju).
--   4) permit_requests.type tambah 'keperluan' (selain sick/leave/other).
--   5) activity_logs — BARU. Jejak aksi administratif (halaman User Control).
--   6) curriculum — BARU. Cakupan materi/kitab PER KELAS.
--   7) materials — BARU. Library file bersama (murattal/kitab/video), lepas
--      dari class_sessions.recording_* (itu rekaman SATU sesi live; ini upload
--      manual staf, class_id opsional).
--
-- Idempotent sedapat mungkin. Jalankan setelah migrasi 1–16.
-- =============================================================================

-- 1) users.role: tambahkan 'dewan_guru' ke CHECK ------------------------------
ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check CHECK (
    role IN ('admin', 'teacher', 'dewan_guru', 'supervisor', 'santri', 'parent')
);

-- 2) class_schedules.room ------------------------------------------------------
ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS room VARCHAR(100);

-- 3)+4) permit_requests: dua tahap + tipe 'keperluan' -------------------------
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'permit_requests' AND column_name = 'status'
    ) THEN
        ALTER TABLE permit_requests RENAME COLUMN status TO pamong_status;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'permit_requests' AND column_name = 'reviewed_by'
    ) THEN
        ALTER TABLE permit_requests RENAME COLUMN reviewed_by TO pamong_by;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'permit_requests' AND column_name = 'reviewed_at'
    ) THEN
        ALTER TABLE permit_requests RENAME COLUMN reviewed_at TO pamong_at;
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'permit_requests' AND column_name = 'review_note'
    ) THEN
        ALTER TABLE permit_requests RENAME COLUMN review_note TO pamong_note;
    END IF;
END $$;

ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS requested_by BIGINT REFERENCES users(id) ON DELETE CASCADE;
UPDATE permit_requests SET requested_by = user_id WHERE requested_by IS NULL;
ALTER TABLE permit_requests ALTER COLUMN requested_by SET NOT NULL;

ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS parent_status VARCHAR(20) NOT NULL DEFAULT 'approved';
ALTER TABLE permit_requests DROP CONSTRAINT IF EXISTS permit_requests_parent_status_check;
ALTER TABLE permit_requests ADD CONSTRAINT permit_requests_parent_status_check
    CHECK (parent_status IN ('pending', 'approved', 'rejected'));

ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS parent_confirmed_by BIGINT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS parent_confirmed_at TIMESTAMPTZ;

ALTER TABLE permit_requests DROP CONSTRAINT IF EXISTS permit_requests_type_check;
ALTER TABLE permit_requests ADD CONSTRAINT permit_requests_type_check
    CHECK (type IN ('sick', 'leave', 'keperluan', 'other'));

DROP INDEX IF EXISTS idx_permit_status;
CREATE INDEX IF NOT EXISTS idx_permit_parent_pending ON permit_requests (parent_status) WHERE parent_status = 'pending';
CREATE INDEX IF NOT EXISTS idx_permit_pamong_pending
    ON permit_requests (pamong_status) WHERE parent_status = 'approved' AND pamong_status = 'pending';

-- 5) activity_logs (halaman User Control) -------------------------------------
CREATE TABLE IF NOT EXISTS activity_logs (
    id              BIGSERIAL PRIMARY KEY,
    actor_id        BIGINT REFERENCES users(id) ON DELETE SET NULL,
    target_user_id  BIGINT REFERENCES users(id) ON DELETE SET NULL,
    action          VARCHAR(50) NOT NULL,
    detail          TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_activity_logs_actor  ON activity_logs (actor_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_activity_logs_target ON activity_logs (target_user_id, created_at DESC);

-- 6) curriculum (cakupan materi/kitab per kelas) ------------------------------
CREATE TABLE IF NOT EXISTS curriculum (
    id            BIGSERIAL PRIMARY KEY,
    class_id      BIGINT NOT NULL REFERENCES classes(id) ON DELETE CASCADE,
    title         VARCHAR(150) NOT NULL,
    description   TEXT,
    scope_start   VARCHAR(100),
    scope_end     VARCHAR(100),
    progress_pct  SMALLINT NOT NULL DEFAULT 0 CHECK (progress_pct BETWEEN 0 AND 100),
    order_index   SMALLINT NOT NULL DEFAULT 0,
    status        VARCHAR(20) NOT NULL DEFAULT 'active' CHECK (
                      status IN ('active', 'completed', 'upcoming')
                  ),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_curriculum_class ON curriculum (class_id, order_index);

-- 7) materials (library file bersama) -----------------------------------------
-- CATATAN: tabel ini TIDAK ada di draft SQL yang diberikan user (hanya
-- disebut di komentar, DDL-nya hilang) — didefinisikan di sini mengikuti
-- mockup "Materials Library" (dashboard_dewan_guru_final: MP3/PDF/link
-- YouTube) + pola StorageService/RustFS yang sudah dipakai class_sessions.
CREATE TABLE IF NOT EXISTS materials (
    id           BIGSERIAL PRIMARY KEY,
    class_id     BIGINT REFERENCES classes(id) ON DELETE SET NULL,
    uploaded_by  BIGINT REFERENCES users(id) ON DELETE SET NULL,
    title        VARCHAR(200) NOT NULL,
    kind         VARCHAR(20) NOT NULL CHECK (kind IN ('audio', 'document', 'video', 'link')),
    file_url     TEXT NOT NULL,
    mime_type    VARCHAR(100),
    file_size    BIGINT,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_materials_class  ON materials (class_id);
CREATE INDEX IF NOT EXISTS idx_materials_recent ON materials (created_at DESC);
