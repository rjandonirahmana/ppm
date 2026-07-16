-- =============================================================================
-- 2.sql — Kelengkapan skema ABSENSI SANTRI PPM AFM
--
-- Migrasi 1.sql sudah bagus untuk master data (users, classes, schedules,
-- sessions, complaints), TAPI inti aplikasi — pencatatan KEHADIRAN — belum ada.
-- Migrasi ini menambah:
--   1) kolom identitas & login pada users (login "username atau email" + NIS + kelas)
--   2) attendances  → INTI: hasil scan RFID/QR/manual per sesi, + verifikasi bertahap
--   3) point_logs   → riwayat poin santri (dashboard "Poin Saya 850")
--   4) permit_requests → izin/sakit (menu "Requests", "Izin - Sakit • Disetujui")
--   5) parent_students → koneksi orang tua ↔ santri (halaman "Data Koneksi Orang Tua")
--   6) sessions     → sesi login server-side (opsional bila tak pakai JWT stateless)
--
-- Idempotent sedapat mungkin (IF NOT EXISTS).  Jalankan setelah 1.sql.
-- =============================================================================


-- 1) IDENTITAS & LOGIN pada users -------------------------------------------------
-- Login memakai "Username atau Email" (lihat desain login), tapi users belum
-- punya kolomnya. Tambah username/email + NIS + cache saldo poin.
--
-- CATATAN DESAIN (masukan: 1 santri bisa masuk BANYAK kelas & jadwal):
--   → keanggotaan kelas TIDAK disimpan sbg users.class_id (itu 1-ke-1, salah).
--     Sumbernya = tabel junction `class_participants` (many-to-many, sudah ada).
--   → Untuk query cepat "kelas UTAMA/homeroom santri", ditambah flag `is_primary`
--     pada class_participants + index parsial → lookup O(1) tanpa denormalisasi.
--   → Orang tua BUKAN tabel terpisah: cukup users.role='parent'; relasinya ke
--     santri lewat junction `parent_students` (di bawah).
ALTER TABLE users ADD COLUMN IF NOT EXISTS username  VARCHAR(50)  UNIQUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS email     VARCHAR(150) UNIQUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS nis       VARCHAR(30)  UNIQUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS points    INTEGER NOT NULL DEFAULT 0;
-- Relasi antar-user TANPA tabel terpisah: untuk role='parent', related_id =
-- user_id SANTRI yang dipantau. (Keputusan: tabel parent digabung ke users.)
ALTER TABLE users ADD COLUMN IF NOT EXISTS related_id BIGINT REFERENCES users(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_users_related ON users (related_id);

CREATE INDEX IF NOT EXISTS idx_users_role ON users (role);

-- Kelas UTAMA (homeroom/angkatan) santri: 1 baris class_participants ditandai
-- is_primary. Query "kelas santri X" = index parsial ini (cepat), tanpa taruh
-- class_id di users. Santri tetap bisa ikut banyak kelas/jadwal lain (baris lain).
ALTER TABLE class_participants
    ADD COLUMN IF NOT EXISTS is_primary BOOLEAN NOT NULL DEFAULT FALSE;
CREATE INDEX IF NOT EXISTS idx_cp_user            ON class_participants (user_id);
CREATE INDEX IF NOT EXISTS idx_cp_primary         ON class_participants (user_id) WHERE is_primary;
CREATE INDEX IF NOT EXISTS idx_cp_class_schedule  ON class_participants (class_schedule_id);
-- Maks 1 kelas utama per santri.
CREATE UNIQUE INDEX IF NOT EXISTS uq_cp_one_primary ON class_participants (user_id) WHERE is_primary;


-- 2) ATTENDANCES (inti absensi) --------------------------------------------------
-- Satu baris = satu kehadiran santri pada sebuah sesi kelas. Sumber scan bisa
-- RFID di gate, QR, atau input manual. Status kehadiran + verifikasi DUA TAHAP:
--   tahap 1 (pamong)      → pamong_status
--   tahap 2 (dewan guru)  → verify_status
-- (lihat halaman "Verifikasi Pamong" dan "Verifikasi Kehadiran Tahap 2").
CREATE TABLE IF NOT EXISTS attendances (
    id                  BIGSERIAL PRIMARY KEY,

    user_id             BIGINT NOT NULL
                        REFERENCES users(id) ON DELETE CASCADE,

    class_session_id    BIGINT
                        REFERENCES class_sessions(id) ON DELETE SET NULL,

    class_schedule_id   BIGINT
                        REFERENCES class_schedules(id) ON DELETE SET NULL,

    -- Perangkat & gerbang tempat scan (mis. "Gate 1").
    device_id           BIGINT
                        REFERENCES rfid_devices(id) ON DELETE SET NULL,
    gate_label          VARCHAR(50),

    scanned_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    status              VARCHAR(20) NOT NULL DEFAULT 'present' CHECK (
                            status IN ('present','late','absent','permit','sick')
                        ),

    method              VARCHAR(20) NOT NULL DEFAULT 'rfid' CHECK (
                            method IN ('rfid','qr','manual')
                        ),

    -- Verifikasi tahap 1 (pamong / supervisor)
    pamong_status       VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (
                            pamong_status IN ('pending','approved','rejected')
                        ),
    pamong_by           BIGINT REFERENCES users(id) ON DELETE SET NULL,
    pamong_at           TIMESTAMPTZ,

    -- Verifikasi tahap 2 (dewan guru) — final
    verify_status       VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (
                            verify_status IN ('pending','approved','rejected')
                        ),
    verified_by         BIGINT REFERENCES users(id) ON DELETE SET NULL,
    verified_at         TIMESTAMPTZ,

    note                TEXT,

    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),

    -- Satu santri satu absensi per sesi (cegah scan ganda).
    UNIQUE (user_id, class_session_id)
);

-- Query panas: riwayat santri (dashboard/riwayat), antrean verifikasi pamong/dewan.
CREATE INDEX IF NOT EXISTS idx_att_user_date   ON attendances (user_id, scanned_at DESC);
CREATE INDEX IF NOT EXISTS idx_att_session     ON attendances (class_session_id);
CREATE INDEX IF NOT EXISTS idx_att_pamong_pending ON attendances (pamong_status) WHERE pamong_status = 'pending';
CREATE INDEX IF NOT EXISTS idx_att_verify_pending ON attendances (verify_status) WHERE verify_status = 'pending';


-- 3) POINT_LOGS (poin santri) ----------------------------------------------------
-- Riwayat perubahan poin (naik/turun). users.points = cache saldo (di-update saat
-- insert log). Kategori untuk analitik dewan guru.
CREATE TABLE IF NOT EXISTS point_logs (
    id          BIGSERIAL PRIMARY KEY,

    user_id     BIGINT NOT NULL
                REFERENCES users(id) ON DELETE CASCADE,

    delta       INTEGER NOT NULL,                 -- +/- poin
    reason      VARCHAR(200) NOT NULL,
    category    VARCHAR(30) NOT NULL DEFAULT 'other' CHECK (
                    category IN ('attendance','discipline','achievement','other')
                ),

    given_by    BIGINT REFERENCES users(id) ON DELETE SET NULL,

    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_point_logs_user ON point_logs (user_id, created_at DESC);


-- 4) PERMIT_REQUESTS (izin / sakit) ---------------------------------------------
-- Santri mengajukan izin/sakit; pamong/dewan guru menyetujui/menolak.
CREATE TABLE IF NOT EXISTS permit_requests (
    id              BIGSERIAL PRIMARY KEY,

    user_id         BIGINT NOT NULL
                    REFERENCES users(id) ON DELETE CASCADE,

    type            VARCHAR(20) NOT NULL CHECK (type IN ('sick','leave','other')),
    reason          TEXT NOT NULL,

    start_date      DATE NOT NULL,
    end_date        DATE,

    attachment_path TEXT,
    mime_type       VARCHAR(100),

    status          VARCHAR(20) NOT NULL DEFAULT 'pending' CHECK (
                        status IN ('pending','approved','rejected')
                    ),
    reviewed_by     BIGINT REFERENCES users(id) ON DELETE SET NULL,
    reviewed_at     TIMESTAMPTZ,
    review_note     TEXT,

    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_permit_user   ON permit_requests (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_permit_status ON permit_requests (status) WHERE status = 'pending';


-- 5) (DIHAPUS) parent_students — relasi orang tua↔santri kini via users.related_id
--    (role='parent' → related_id = user_id santri). Lihat bagian 1 di atas.
--    DB lama yang terlanjur punya tabel ini dibereskan migrasi 4_parent_related.sql.


-- 6) SESSIONS (login server-side, opsional) -------------------------------------
-- Dipakai bila memilih sesi cookie server-side (bukan JWT stateless).
-- gen_random_uuid() tersedia bawaan sejak Postgres 13.
CREATE TABLE IF NOT EXISTS sessions (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at  TIMESTAMPTZ NOT NULL,
    user_agent  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions (user_id);
