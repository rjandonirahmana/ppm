-- =============================================================================
-- 25_materi_category_units.sql — Materi (books) berkategori + progres per-unit
-- 3-status.
--
-- books.category: 'quran' | 'hadist'.
--   • hadist → unit = HALAMAN; total_pages = jumlah halaman.
--   • quran  → unit = AYAT per SURAT; surahs JSONB = [{"name","ayat"}]; total_pages
--     di-set = TOTAL ayat (jumlah semua ayat) supaya jadi "total unit" seragam.
--
-- academic_user.unit_status JSONB = peta per-unit 3-status yang diisi SANTRI:
--   key = "<halaman>" (hadist) atau "<surahIdx>:<ayat>" (quran); value = 1
--   (setengah) | 2 (penuh). Unit tak ada di peta = KOSONG. percentage dihitung
--   ulang = ROUND(SUM(value) / (total_pages*2) * 100).
--
-- Kolom lama missing_pages DIBIARKAN (tak dipakai). Idempotent. Setelah 1–24.
-- =============================================================================

ALTER TABLE books ADD COLUMN IF NOT EXISTS category VARCHAR(20) NOT NULL DEFAULT 'hadist';
ALTER TABLE books ADD COLUMN IF NOT EXISTS surahs JSONB NOT NULL DEFAULT '[]'::jsonb;

ALTER TABLE academic_user ADD COLUMN IF NOT EXISTS unit_status JSONB NOT NULL DEFAULT '{}'::jsonb;
