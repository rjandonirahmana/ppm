-- =============================================================================
-- 10_schedule_category.sql — Kategori PER-JADWAL (bukan hanya per-kelas).
--
-- Kenapa: satu kelas bisa punya BEBERAPA jadwal berbeda jenis kegiatan (mis.
-- "Halaqah Subuh" punya jadwal Pengajian DAN jadwal Sholat Berjamaah). Gerbang
-- rekam suara (hanya kategori "Pengajian" boleh siaran — lihat
-- models::category_allows_recording) jadi lebih akurat kalau categorynya per
-- JADWAL, bukan disamaratakan ke seluruh kelas.
--
-- TETAP teks bebas (konsisten classes.category, migrasi 6) — dropdown UI diisi
-- DISTINCT category yang ada + boleh ketik baru.
--
-- Resolusi kategori efektif sebuah sesi: COALESCE(jadwal.category, kelas.category)
-- — jadwal override kelas; sesi ad-hoc tanpa jadwal jatuh ke kategori kelas.
--
-- Idempotent. Jalankan setelah migrasi 1–9.
-- =============================================================================

ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS category VARCHAR(50);

CREATE INDEX IF NOT EXISTS idx_schedules_category ON class_schedules (category)
    WHERE category IS NOT NULL;
