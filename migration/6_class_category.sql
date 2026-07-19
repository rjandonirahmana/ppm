-- =============================================================================
-- 6_class_category.sql — Kategori kelas FLEKSIBEL (teks bebas).
--
-- Keputusan desain: kategori kelas TIDAK dibatasi enum. Cukup satu kolom teks;
-- dropdown di UI diisi dari DISTINCT category yang sudah ada + boleh ketik baru,
-- jadi menambah kategori (Cepatan/Lambatan/Hadist Besar/Test Kediri/…) tak perlu
-- migration lagi. "Semua/All" hanya opsi filter, tidak disimpan.
--
-- Guru TIDAK disimpan di classes (berganti tiap sesi → class_sessions.teacher_id).
-- Jadwal (class_schedules) = pola rutin; Sesi (class_sessions) = pertemuan nyata
-- bertanggal — keduanya sudah ada di migrasi 1, tak ada kolom baru di sini.
--
-- Idempotent. Jalankan setelah migrasi 1–5.
-- =============================================================================

ALTER TABLE classes ADD COLUMN IF NOT EXISTS category VARCHAR(50);

CREATE INDEX IF NOT EXISTS idx_classes_category ON classes (category) WHERE category IS NOT NULL;
