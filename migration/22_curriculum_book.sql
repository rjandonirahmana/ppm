-- =============================================================================
-- 22_curriculum_book.sql — Kurikulum kelas (migrasi 17) bisa DITAUTKAN ke satu
-- materi terdaftar (tabel `books`, mis. "Sahih Bukhari"/"Al-Qur'an") — opsional.
-- Materi bebas-teks tetap boleh (title diisi manual); book_id hanya penanda
-- referensi ke daftar materi resmi supaya konsisten & bisa di-query.
--
-- ON DELETE SET NULL: materi dihapus → tautan lepas, baris kurikulum tetap.
-- Idempotent. Jalankan setelah migrasi 1–21.
-- =============================================================================

ALTER TABLE curriculum
    ADD COLUMN IF NOT EXISTS book_id BIGINT REFERENCES books(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_curriculum_book ON curriculum (book_id) WHERE book_id IS NOT NULL;
