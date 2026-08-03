-- =============================================================================
-- 55_activity_photo_fit.sql — Mode tampilan foto kegiatan: isi bingkai vs utuh.
--
-- LATAR: migrasi 54 menambahkan titik fokus & perbesaran, tapi semuanya masih
-- bertumpu pada `object-fit: cover` — foto SELALU memenuhi bingkai, dan itu
-- berarti selalu ada bagian yang terpotong. Untuk foto yang sangat tegak atau
-- sangat lebar, memindahkan titik fokus saja tidak cukup: apa pun bidikannya,
-- sebagian gambar tetap hilang. Kadang yang dibutuhkan justru foto UTUH,
-- meskipun harus menyisakan ruang di kiri-kanan atau atas-bawah.
--
--   fit = 'cover'   : foto memenuhi bingkai, sisi yang lebih panjang terpotong.
--                     Titik fokus menentukan bagian mana yang dipertahankan.
--                     BAWAAN — sama persis dengan perilaku sebelum migrasi ini,
--                     jadi seluruh foto yang sudah ada tampil tak berubah.
--   fit = 'contain' : foto tampil UTUH di dalam bingkai. Ruang sisa diisi versi
--                     buram foto itu sendiri (bukan blok abu-abu) supaya kartu
--                     tetap terlihat penuh dan menyatu.
--
-- Disimpan per FOTO, bukan per halaman: dalam satu galeri biasanya bercampur
-- foto lanskap yang enak dipotong penuh dan foto potret yang justru harus utuh.
--
-- Idempotent. Jalankan setelah migrasi 1–54.
-- =============================================================================

ALTER TABLE activity_photos
    ADD COLUMN IF NOT EXISTS fit TEXT NOT NULL DEFAULT 'cover';

-- Nilai dijaga di database, bukan hanya di formulir: nilai lain akan lolos ke
-- atribut CSS dan menghasilkan tampilan yang tak bisa dijelaskan asalnya.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'activity_photos_fit_valid'
    ) THEN
        ALTER TABLE activity_photos
            ADD CONSTRAINT activity_photos_fit_valid
            CHECK (fit IN ('cover', 'contain'));
    END IF;
END $$;

COMMENT ON COLUMN activity_photos.fit IS
    '''cover'' = penuhi bingkai (terpotong, posisi diatur focus_x/focus_y); ''contain'' = foto utuh, ruang sisa diisi versi buram foto.';
