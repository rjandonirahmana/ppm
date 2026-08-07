-- =============================================================================
-- 66_izin_per_jam.sql — Izin bisa PER JAM, bukan hanya per hari.
--
-- KENAPA
-- `permit_requests` hanya punya start_date/end_date, jadi setiap izin otomatis
-- memakan HARI PENUH. Kenyataannya santri kerap izin sebentar — keluar 09:00
-- s/d 11:00 untuk urusan tertentu — dan hari itu ia tetap mengikuti kelas
-- subuh, piket sore, dan apel malam. Dengan izin sehari penuh, seluruh sesi
-- hari itu ikut ditandai izin: kehadiran yang benar-benar terjadi hilang dari
-- catatan, dan poin kedisiplinannya ikut terpotong.
--
-- BENTUKNYA
--   start_time/end_time NULL  → izin SEHARI PENUH (perilaku lama, tak berubah)
--   start_time/end_time diisi → hanya sesi yang JAMNYA BERSINGGUNGAN dengan
--                               rentang itu yang dianggap terlewat
--
-- Jamnya berlaku untuk SETIAP hari dalam rentang, bukan sekali di ujung: izin
-- "09:00–11:00, 3 hari" berarti tiga kali dua jam. Itu bentuk yang cocok dengan
-- jadwal pesantren yang berulang tiap hari, dan yang orang maksud saat menulis
-- jam pada formulir izin.
--
-- Idempotent. Jalankan setelah migrasi 1–65.
-- =============================================================================

ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS start_time TIME;
ALTER TABLE permit_requests ADD COLUMN IF NOT EXISTS end_time   TIME;

-- Keduanya diisi, atau keduanya kosong. Satu ujung saja tak punya arti: "izin
-- mulai jam 9" tanpa akhir tak bisa dibandingkan dengan jam sesi mana pun.
ALTER TABLE permit_requests DROP CONSTRAINT IF EXISTS chk_permit_jam;
ALTER TABLE permit_requests ADD CONSTRAINT chk_permit_jam CHECK (
    (start_time IS NULL AND end_time IS NULL)
 OR (start_time IS NOT NULL AND end_time IS NOT NULL AND end_time > start_time)
);

-- Verifikasi:
--   SELECT id, start_date, end_date, start_time, end_time FROM permit_requests
--    ORDER BY id DESC LIMIT 10;
