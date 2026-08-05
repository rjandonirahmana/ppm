-- =============================================================================
-- 60_drop_derived_and_unused.sql — Buang kolom yang nilainya kini DITURUNKAN,
-- dan sisa penanda posisi di jadwal.
--
-- 1) curriculum.progress_pct & curriculum.status
--    Sejak migrasi 59 keduanya dihitung ulang tiap kali dibaca, dari SATU
--    fakta: posisi terakhir (`current_surah`/`current_unit`) diukur terhadap
--    rentang materinya.
--        persen = (posisi − awal + 1) / (akhir − awal + 1)
--        status = 100% → completed, >0% → active, 0% → upcoming
--    Menyimpannya juga di kolom berarti dua sumber untuk satu fakta — dan
--    keduanya SUDAH menyimpang: baris Al Fatihah masih tersimpan 'completed'
--    padahal posisinya belum diisi sama sekali. Kolom yang tak pernah dibaca
--    tapi masih terlihat di skema adalah jebakan bagi siapa pun yang membaca
--    tabel ini nanti dan mengira nilainya berlaku.
--
-- 2) class_schedules.current_surah & current_unit
--    Migrasi 57 sempat menaruh posisi di jadwal; migrasi 59 memindahkannya ke
--    kurikulum karena hanya di sana ada rentang untuk mengukurnya. Kolomnya
--    ditinggalkan sebagai jalan mundur — sekarang perpindahan itu sudah
--    terbukti jalan, jadi tak perlu lagi. `current_book_id` TETAP: maknanya
--    "jadwal ini sedang membahas materi yang mana", dan itu masih dipakai.
--
-- Kolom yang dibuang beserta isinya tak bisa dikembalikan. IF EXISTS membuat
-- pengulangan aman. Jalankan setelah migrasi 1–59.
-- =============================================================================

ALTER TABLE curriculum
    DROP COLUMN IF EXISTS progress_pct,
    DROP COLUMN IF EXISTS status;

-- Verifikasi:
--   SELECT column_name FROM information_schema.columns
--    WHERE table_name IN ('curriculum','class_schedules') ORDER BY table_name, ordinal_position;
