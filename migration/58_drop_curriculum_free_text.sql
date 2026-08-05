-- =============================================================================
-- 58_drop_curriculum_free_text.sql — Buang kolom kurikulum yang sudah tak dipakai.
--
-- Sejak migrasi 57 kurikulum WAJIB menunjuk materi terdaftar (`book_id`), dan
-- sejak itu:
--   • `title`       → judulnya diambil dari `books.title`;
--   • `description` → tak pernah diisi lagi (materi sudah punya identitasnya);
--   • `scope_start` / `scope_end` → digantikan rentang ANGKA
--     (start_surah/start_unit/end_surah/end_unit) yang bisa divalidasi terhadap
--     jumlah halaman/ayat materinya.
--
-- Ketiga kolom di bawah sudah tidak dibaca maupun ditulis kode mana pun
-- (repository::class_curriculum, create_curriculum, update_curriculum sudah
-- dibersihkan). Sebelum menjalankan ini, PASTIKAN tak ada baris yang masih
-- bergantung padanya:
--
--   SELECT id, title, book_id, scope_start, scope_end
--     FROM curriculum WHERE book_id IS NULL;
--
-- Baris ber-`book_id` NULL adalah sisa sebelum aturan "wajib tertaut" — ia
-- kehilangan satu-satunya keterangan rentangnya bila kolom ini dibuang. Saat
-- migrasi ini dibuat, query di atas mengembalikan 0 baris.
--
-- `title` SENGAJA DIPERTAHANKAN: kolomnya NOT NULL sejak migrasi 17, diisi
-- otomatis dari judul materi, dan berguna sebagai jejak bila sebuah materi
-- kelak dihapus (book_id → NULL karena ON DELETE SET NULL).
--
-- TIDAK idempotent-aman untuk di-rollback: kolom yang dibuang beserta isinya
-- tak bisa dikembalikan. IF EXISTS membuat pengulangan menjalankannya aman.
-- Jalankan setelah migrasi 1–57.
-- =============================================================================

ALTER TABLE curriculum
    DROP COLUMN IF EXISTS description,
    DROP COLUMN IF EXISTS scope_start,
    DROP COLUMN IF EXISTS scope_end;

-- Verifikasi:
--   SELECT column_name FROM information_schema.columns
--    WHERE table_name = 'curriculum' ORDER BY ordinal_position;
