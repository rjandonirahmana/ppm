-- =============================================================================
-- 41_session_target_actual.sql — Target vs Aktual materi per sesi.
-- Sebelumnya class_sessions hanya punya SATU materi (book_id + book_pages,
-- migrasi 20) = materi AKTUAL. Sekarang tambah:
--   • target_book_id + target_pages  → materi yang DIRENCANAKAN untuk sesi ini
--   • actual_detail (teks)           → catatan bebas ayat/hadith yang BENAR-BENAR
--                                       dibahas (mis. "An-Naba' 1-20" / "Hadith 5-8")
-- book_id/book_pages (migrasi 20) tetap = materi AKTUAL (buku + halaman).
-- Idempotent. Setelah migrasi 1–40.
-- =============================================================================

ALTER TABLE class_sessions
    ADD COLUMN IF NOT EXISTS target_book_id BIGINT REFERENCES books(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS target_pages   JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS actual_detail  TEXT  NOT NULL DEFAULT '';

CREATE INDEX IF NOT EXISTS idx_class_sessions_target_book
    ON class_sessions (target_book_id) WHERE target_book_id IS NOT NULL;
