-- =============================================================================
-- 16_class_golongan.sql — Golongan kelas (Bacaan / Makna / …), TERPISAH dari
-- category (migrasi 6).
--
-- Kenapa: category sudah dipakai sebagai LABEL kelas itu sendiri (mis.
-- "Lambatan", "Cepatan", "Hadist Besar", "Mubalegh") — tapi label-label itu
-- sebenarnya jatuh ke DUA SUMBU klasifikasi berbeda yang pesantren pakai:
--   • Bacaan — Lambatan / Cepatan (kecepatan baca)
--   • Makna  — Hadist Besar / Mubalegh / dst. (kelas makna/pemahaman)
-- Seorang santri lazimnya terdaftar di SATU kelas per golongan (satu kelas
-- Bacaan + satu kelas Makna) — dua baris class_participants berbeda class_id.
-- Tanpa kolom ini, tak ada cara membedakan sumbu mana yang dipegang sebuah
-- category, dan daftar santri hanya bisa menampilkan SATU kelas per santri
-- (lihat repository::points_board — LIMIT 1) walau santri sebenarnya ikut dua.
--
-- TETAP teks bebas (konsisten filosofi category, migrasi 6) — dropdown UI diisi
-- DISTINCT golongan yang ada + boleh ketik baru. NULL/kosong = kelas di luar
-- sistem dua-sumbu ini (mis. "Pengajian"/kelas umum lain), tak wajib diisi.
--
-- Idempotent. Jalankan setelah migrasi 1–15.
-- =============================================================================

ALTER TABLE classes ADD COLUMN IF NOT EXISTS golongan VARCHAR(50);

CREATE INDEX IF NOT EXISTS idx_classes_golongan ON classes (golongan) WHERE golongan IS NOT NULL;
