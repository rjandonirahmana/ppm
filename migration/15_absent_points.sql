-- =============================================================================
-- 15_absent_points.sql — Poin ALPA (absent) bisa dikustomisasi PER JADWAL,
-- sama seperti late_points (migrasi 13) tapi untuk status 'absent'.
--
-- BEDA SEMANTIK dgn late_points: late_points dipakai LANGSUNG sebagai delta
-- bertanda (boleh negatif MAUPUN positif — telat di jadwal longgar bisa saja
-- tetap netral/naik tipis). absent_points SELALU MAGNITUDE POSITIF — kode
-- menghitung `points = points - absent_points` (lihat repository/attendance.rs
-- run_auto_absent), bukan ditambah sebagai delta negatif langsung.
--
-- NULL = pakai default global 15 (models::attendance::point_rule "absent").
-- Diisi (mis. 20) → override HANYA utk status 'absent' pada jadwal itu.
--
-- Idempotent. Jalankan setelah migrasi 1–14.
-- =============================================================================

ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS absent_points SMALLINT;
