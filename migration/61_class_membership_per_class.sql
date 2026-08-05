-- =============================================================================
-- 61_class_membership_per_class.sql — Keanggotaan kelas jadi PER KELAS,
-- bukan per jadwal.
--
-- MASALAH
-- `class_participants.class_schedule_id` NOT NULL memaksa setiap penambahan
-- santri memilih SATU jadwal ("Tempatkan pada jadwal"). Itu tak sesuai
-- kenyataan: santri masuk KELAS, dan kelas itu punya beberapa jadwal yang
-- semuanya ia ikuti. Akibat model lama:
--   • santri yang mengikuti semua jadwal harus didaftarkan berkali-kali;
--   • yang terdaftar di satu jadwal saja TIDAK dianggap peserta jadwal lain —
--     ia tak pernah dialpakan di sesi jadwal itu, dan tak muncul di daftar
--     absensinya, tanpa ada yang sadar;
--   • pengelola dipaksa menjawab pertanyaan yang tak punya jawaban benar.
--
-- SESUDAHNYA
-- Satu baris per (kelas, santri). Semua jadwal kelas otomatis berlaku
-- untuknya. Semua query yang dulu menyambung lewat `class_schedule_id` kini
-- menyambung lewat `class_id` — dan semuanya jadi lebih pendek, termasuk
-- `run_auto_absent` dan daftar wali kelas di permits yang sebelumnya sudah
-- memakai `OR cp.class_id = ...` sebagai jalur cadangan.
--
-- URUTAN PENTING: duplikat dirapikan DULU, baru UNIQUE diganti — kalau
-- terbalik, pembuatan constraint gagal pada kelas yang santrinya terdaftar di
-- lebih dari satu jadwal.
--
-- Idempotent. Jalankan setelah migrasi 1–60.
-- =============================================================================

-- ═ 1) Rapikan duplikat: sisakan baris TERTUA per (kelas, santri) ════════════
DELETE FROM class_participants a
 USING class_participants b
 WHERE a.id > b.id
   AND a.class_id = b.class_id
   AND a.user_id  = b.user_id;

-- ═ 2) Kunci keanggotaan per (kelas, santri) ═════════════════════════════════
ALTER TABLE class_participants
    DROP CONSTRAINT IF EXISTS class_participants_class_id_user_id_class_schedule_id_key;

ALTER TABLE class_participants
    DROP CONSTRAINT IF EXISTS uq_class_participant;
ALTER TABLE class_participants
    ADD CONSTRAINT uq_class_participant UNIQUE (class_id, user_id);

-- ═ 3) Kolom jadwal tak lagi bermakna ════════════════════════════════════════
-- Index idx_cp_class_schedule ikut terbuang bersama kolomnya.
ALTER TABLE class_participants
    DROP COLUMN IF EXISTS class_schedule_id;

-- Verifikasi:
--   SELECT class_id, user_id, count(*) FROM class_participants
--    GROUP BY class_id, user_id HAVING count(*) > 1;   -- harus kosong
