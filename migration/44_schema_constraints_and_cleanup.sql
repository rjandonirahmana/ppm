-- =============================================================================
-- 44_schema_constraints_and_cleanup.sql — Database schema cleanup & validation
--
-- Perbaikan integritas data:
-- 1) Sinkronisasi users.role dengan VALID_ROLES di Rust (teacher → dewan_guru)
-- 2) Tambahkan CHECK constraints untuk validasi nilai (points range, gender, dll)
-- 3) Drop kolom dead yang tidak dipakai (class_schedules.room)
-- 4) Perbaiki foreign key cascade policies untuk data historis
--
-- Idempotent (tiap ADD CONSTRAINT didahului DROP IF EXISTS — tanpa itu run
-- kedua gagal). Jalankan setelah migrasi 1–43.
-- =============================================================================

-- ═ 1) USERS.ROLE — Sinkronisasi dengan migration 36/38 ═══════════════════════
-- Migration 36/38 sudah ubah 'teacher' → 'dewan_guru', tapi disini kita
-- pastikan constraint benar-benar enforce (redundant check tapi aman).

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN ('admin', 'ketua', 'dewan_guru', 'supervisor', 'santri', 'santri_finance', 'parent'));

-- ═ 2) USERS.POINTS — Batas range untuk prevent overflow ═══════════════════
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_points_range;
ALTER TABLE users ADD CONSTRAINT chk_users_points_range
    CHECK (points BETWEEN -10000 AND 10000);

-- ═ 3) USERS.GENDER — Validasi format (L/P saja) ═════════════════════════════
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_gender;
ALTER TABLE users ADD CONSTRAINT chk_users_gender
    CHECK (gender IS NULL OR gender IN ('L', 'P'));

-- ═ 4) CLASS_SCHEDULES — Validasi point values (non-negative) ════════════════
ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS chk_late_points_nonneg;
ALTER TABLE class_schedules ADD CONSTRAINT chk_late_points_nonneg
    CHECK (late_points IS NULL OR late_points >= 0);

ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS chk_absent_points_nonneg;
ALTER TABLE class_schedules ADD CONSTRAINT chk_absent_points_nonneg
    CHECK (absent_points IS NULL OR absent_points >= 0);

ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS chk_izin_points_nonneg;
ALTER TABLE class_schedules ADD CONSTRAINT chk_izin_points_nonneg
    CHECK (izin_points IS NULL OR izin_points >= 0);

-- ═ 5) BILLS — Soft delete support (audit trail untuk keuangan) ═══════════════
-- Catatan: INSERT default = NULL, query default WHERE deleted_at IS NULL
ALTER TABLE bills ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ;

-- ═ 6) CLASS_SCHEDULES — Drop kolom dead (room) ═══════════════════════════════
-- room sudah di-handle via room_id (FK ke rfid_devices).
-- Pastikan NO data di kolom room sebelum drop (lihat query di bawah).
-- SELECT COUNT(*) FROM class_schedules WHERE room IS NOT NULL AND room != '';
-- Kalau ada data, FIRST backfill ke room_id via lookup.

ALTER TABLE class_schedules DROP COLUMN IF EXISTS room;

-- ═ 7) CLASS_SESSIONS — Foreign key cascade policy ═════════════════════════════
-- Jika kelas dihapus, apakah sesi historis (audit) ikut terhapus?
-- Rekomendasi: RESTRICT (jangan bisa delete kelas ada sesi) atau SET NULL.
-- Untuk saat ini: hanya dokumentasi; tidak mengubah constraint existing.
-- ALTER TABLE class_sessions DROP CONSTRAINT IF EXISTS class_sessions_class_id_fkey;
-- ALTER TABLE class_sessions ADD CONSTRAINT class_sessions_class_id_fkey
--     FOREIGN KEY (class_id) REFERENCES classes(id) ON DELETE RESTRICT;
-- ↑ Uncomment jika ingin enforce RESTRICT (lebih aman untuk audit trail).

-- ═ 8) VALIDATE: Ensure no regressions ═════════════════════════════════════════
-- Jalankan queries di bawah di staging SEBELUM apply production:

-- Verify semua users punya valid role setelah constraint ditambah:
-- SELECT id, role FROM users WHERE role NOT IN ('admin', 'ketua', 'dewan_guru', 'supervisor', 'santri', 'santri_finance', 'parent');

-- Verify semua points dalam range:
-- SELECT id, points FROM users WHERE points < -10000 OR points > 10000;

-- Verify semua gender valid (atau NULL):
-- SELECT id, gender FROM users WHERE gender IS NOT NULL AND gender NOT IN ('L', 'P');

-- Verify point values di schedule valid:
-- SELECT id, late_points, absent_points, izin_points FROM class_schedules
-- WHERE late_points < 0 OR absent_points < 0 OR izin_points < 0;
