-- =============================================================================
-- 13_late_points.sql — Poin TERLAMBAT bisa dikustomisasi PER JADWAL.
--
-- Kenapa per-JADWAL (bukan per-kelas): sama seperti category (migrasi 10) —
-- satu kelas bisa punya jadwal Sholat (telat = pelanggaran, poin berkurang)
-- DAN jadwal Pengajian (telat = masih dianggap wajar, poin tetap netral/naik
-- tipis). Kalau taruh di level kelas, tak bisa beda per jenis kegiatan.
--
-- NULL = pakai default global point_rule("late") = +2 (models::attendance).
-- Diisi (mis. -5) → override HANYA utk status 'late' pada jadwal itu; status
-- lain (present/absent/permit/sick) tetap pakai aturan global.
--
-- Idempotent. Jalankan setelah migrasi 1–12.
-- =============================================================================

ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS late_points SMALLINT;
