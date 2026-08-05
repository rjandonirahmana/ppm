-- =============================================================================
-- 59_curriculum_position.sql — Posisi "sudah sampai mana" pindah ke KURIKULUM.
--
-- Migrasi 57 menaruh penanda posisi di `class_schedules`. Ternyata tempatnya
-- kurang tepat: yang punya RENTANG (start..end) adalah baris kurikulum, jadi
-- "sudah sampai ayat/halaman berapa" hanya bermakna bila diukur terhadap
-- rentang itu — dan progresnya pun milik kurikulum, bukan milik jadwal.
--
-- Dengan posisi di sini, tiga hal jadi satu turunan dari SATU angka:
--   • progres persen  = (posisi − awal + 1) / (akhir − awal + 1)
--   • status          = 100% → 'completed', >0% → 'active', 0% → 'upcoming'
--   • label "sampai mana" di kartu kurikulum
-- Tak ada lagi persen yang diketik tangan lalu basi karena lupa diperbarui.
--
-- `class_schedules.current_book_id` TETAP dipakai — maknanya kini menyempit
-- jadi "jadwal ini sedang membahas materi yang mana", tanpa ikut menyimpan
-- posisi. `current_surah`/`current_unit` di sana jadi tak terpakai; sengaja
-- BELUM dibuang supaya migrasi ini tetap bisa dibalik bila ternyata keliru.
--
-- Idempotent. Jalankan setelah migrasi 1–58.
-- =============================================================================

ALTER TABLE curriculum
    ADD COLUMN IF NOT EXISTS current_surah SMALLINT,
    ADD COLUMN IF NOT EXISTS current_unit  INTEGER;

-- Nomor mulai dari 1; batas ATAS tergantung materinya, jadi dicek di service.
ALTER TABLE curriculum DROP CONSTRAINT IF EXISTS chk_curriculum_current_positive;
ALTER TABLE curriculum ADD CONSTRAINT chk_curriculum_current_positive CHECK (
    (current_unit  IS NULL OR current_unit  >= 1) AND
    (current_surah IS NULL OR current_surah >= 1)
);

-- Pindahkan posisi yang sudah terlanjur diisi di jadwal (migrasi 57) ke baris
-- kurikulum yang materinya sama di kelas yang sama. Bila satu materi dipegang
-- beberapa jadwal, yang PALING MAJU yang dipakai — itu yang mewakili sejauh
-- mana kelas sudah berjalan.
UPDATE curriculum cu
   SET current_surah = m.surah,
       current_unit  = m.unit
  FROM (
        SELECT cs.class_id,
               cs.current_book_id AS book_id,
               (ARRAY_AGG(cs.current_surah ORDER BY cs.current_surah DESC NULLS LAST,
                                                    cs.current_unit  DESC NULLS LAST))[1] AS surah,
               MAX(cs.current_unit) AS unit
          FROM class_schedules cs
         WHERE cs.current_book_id IS NOT NULL AND cs.current_unit IS NOT NULL
         GROUP BY cs.class_id, cs.current_book_id
       ) m
 WHERE cu.class_id = m.class_id
   AND cu.book_id  = m.book_id
   AND cu.current_unit IS NULL;

-- Verifikasi:
--   SELECT id, class_id, title, start_unit, current_unit, end_unit, status
--     FROM curriculum ORDER BY id;
