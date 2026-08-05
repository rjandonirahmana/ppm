-- =============================================================================
-- 57_curriculum_range_and_schedule_progress.sql
--
-- Dua hal, keduanya soal "materi apa, sampai mana":
--
-- 1) KURIKULUM KELAS punya RENTANG TERSTRUKTUR, bukan teks bebas.
--    `scope_start`/`scope_end` (migrasi 17) VARCHAR(100) menerima apa saja —
--    hasilnya isian seperti "juz 1" → "juz 15" yang tak bisa divalidasi, tak
--    bisa dibandingkan dengan jumlah unit materinya, dan tak bisa dihitung.
--    Sekarang rentangnya angka, dan bentuknya mengikuti JENIS materi:
--      • hadist → halaman:  start_unit .. end_unit
--      • quran  → ayat:     (start_surah, start_unit) .. (end_surah, end_unit)
--                 surat disimpan sebagai INDEKS 1-based ke `books.surahs`,
--                 sehingga rentang boleh melintasi surat.
--    `start_unit`/`end_unit` sengaja SATU pasang kolom untuk kedua jenis —
--    keduanya "nomor unit", cuma satuannya beda — supaya tak ada dua pasang
--    kolom yang harus dijaga saling eksklusif.
--
--    `scope_start`/`scope_end` TIDAK dihapus: baris lama yang belum tertaut
--    materi masih memakainya, dan membuangnya berarti kehilangan satu-satunya
--    keterangan yang mereka punya.
--
-- 2) JADWAL menyimpan POSISI YANG SEDANG BERJALAN.
--    `class_schedules` sama sekali belum punya kolom materi. Penanda ini yang
--    menjawab "sekarang sedang materi apa, ayat/halaman berapa" untuk jadwal
--    rutin yang berjalan berminggu-minggu. Sengaja di jadwal, bukan di sesi:
--    ia POINTER yang maju, bukan catatan per pertemuan.
--
-- ON DELETE SET NULL: materi dihapus → penanda lepas, jadwalnya tetap utuh.
-- Idempotent. Jalankan setelah migrasi 1–56.
-- =============================================================================

-- ═ 1) Rentang kurikulum ═════════════════════════════════════════════════════
ALTER TABLE curriculum
    ADD COLUMN IF NOT EXISTS start_surah SMALLINT,
    ADD COLUMN IF NOT EXISTS start_unit  INTEGER,
    ADD COLUMN IF NOT EXISTS end_surah   SMALLINT,
    ADD COLUMN IF NOT EXISTS end_unit    INTEGER;

-- Nomor unit/surat mulai dari 1. Batas ATAS tidak dicek di sini karena
-- bergantung pada materi yang ditunjuk (books.total_pages / panjang surat) —
-- itu divalidasi di service, tempat materinya diketahui.
ALTER TABLE curriculum DROP CONSTRAINT IF EXISTS chk_curriculum_units_positive;
ALTER TABLE curriculum ADD CONSTRAINT chk_curriculum_units_positive CHECK (
    (start_unit  IS NULL OR start_unit  >= 1) AND
    (end_unit    IS NULL OR end_unit    >= 1) AND
    (start_surah IS NULL OR start_surah >= 1) AND
    (end_surah   IS NULL OR end_surah   >= 1)
);

-- Pindahkan rentang lama yang KEBETULAN sudah berupa angka murni (mis. "1" →
-- "20") ke kolom baru. Yang bukan angka (mis. "juz 1") dibiarkan — nilainya
-- tetap terbaca di scope_start/scope_end sampai pengelola menautkannya ke
-- materi dan mengisi rentangnya.
UPDATE curriculum
   SET start_unit = NULLIF(regexp_replace(scope_start, '\D', '', 'g'), '')::int
 WHERE start_unit IS NULL
   AND scope_start ~ '^\s*\d+\s*$';

UPDATE curriculum
   SET end_unit = NULLIF(regexp_replace(scope_end, '\D', '', 'g'), '')::int
 WHERE end_unit IS NULL
   AND scope_end ~ '^\s*\d+\s*$';

-- ═ 2) Posisi berjalan di jadwal ═════════════════════════════════════════════
ALTER TABLE class_schedules
    ADD COLUMN IF NOT EXISTS current_book_id BIGINT REFERENCES books(id) ON DELETE SET NULL,
    ADD COLUMN IF NOT EXISTS current_surah   SMALLINT,
    ADD COLUMN IF NOT EXISTS current_unit    INTEGER;

ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS chk_schedule_current_positive;
ALTER TABLE class_schedules ADD CONSTRAINT chk_schedule_current_positive CHECK (
    (current_unit  IS NULL OR current_unit  >= 1) AND
    (current_surah IS NULL OR current_surah >= 1)
);

CREATE INDEX IF NOT EXISTS idx_schedule_current_book
    ON class_schedules (current_book_id) WHERE current_book_id IS NOT NULL;

-- Verifikasi:
--   SELECT id, title, book_id, scope_start, start_unit, scope_end, end_unit
--     FROM curriculum ORDER BY id;
