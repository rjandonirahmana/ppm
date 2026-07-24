-- =============================================================================
-- 20_session_book_material.sql — Materi buku per SESI (beda dari `curriculum`
-- migrasi 17 yang bebas-teks per KELAS, dan beda dari `academic_user` migrasi
-- 18 yang progres per SANTRI). Di sini: sesi tertentu boleh menunjuk SATU
-- buku (books.id, opsional — sesi non-mengaji spt Sholat tak butuh) + rentang
-- halaman yang dibahas pada sesi itu.
--
-- book_pages JSONB (bukan 2 kolom integer): konsisten dgn pola
-- `academic_user.missing_pages` (array pasangan [awal,akhir], mis.
-- [[11,20],[45,50]]) — sesi bisa membahas beberapa rentang tak berurutan
-- (mis. ulang halaman lama + lanjut baru), dan bentuk ini query-friendly
-- (jsonb_array_length dst) tanpa perlu tabel terpisah untuk kebutuhan simpel
-- ini. Diparse/diformat di service layer dari SATU kotak teks "11-20, 45-50"
-- (reuse parse_missing_pages/format_missing_pages, service/books.rs).
--
-- Idempotent. Jalankan setelah migrasi 1–19.
-- =============================================================================

ALTER TABLE class_sessions
    ADD COLUMN IF NOT EXISTS book_id BIGINT REFERENCES books(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS book_pages JSONB NOT NULL DEFAULT '[]'::jsonb;

CREATE INDEX IF NOT EXISTS idx_class_sessions_book ON class_sessions (book_id) WHERE book_id IS NOT NULL;
