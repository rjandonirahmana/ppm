-- =============================================================================
-- 54_activity_photo_focus.sql — Titik fokus & zoom foto kegiatan.
--
-- LATAR: foto galeri ditampilkan dengan `object-fit: cover` pada DUA rasio yang
-- berbeda — beranda publik memakai 3:4 (tegak), grid pengelola memakai 1:1.
-- `cover` selalu memotong dari TENGAH, jadi foto yang panjang ke atas atau lebar
-- ke samping kehilangan bagian yang justru penting: wajah di tepi atas terpotong,
-- kegiatan di sisi kanan hilang. Tidak ada cara memperbaikinya selain memotong
-- ulang fotonya di luar aplikasi lalu mengunggah lagi.
--
-- Alih-alih menyimpan hasil potongan (yang akan mengunci satu rasio dan merusak
-- salah satu dari dua tampilan itu), yang disimpan adalah CARA MEMANDANGNYA:
-- titik mana pada foto yang harus tetap terlihat, dan seberapa dekat. Berkas
-- aslinya tak pernah disentuh. Nilai yang sama dipakai di kedua rasio dan bisa
-- diubah kapan saja tanpa mengunggah ulang.
--
--   focus_x / focus_y : 0..1, posisi titik fokus relatif terhadap lebar/tinggi
--                       foto. 0.5/0.5 = tengah — persis perilaku lama, sehingga
--                       SEMUA foto yang sudah ada tampil sama seperti sebelumnya.
--                       Diterapkan sebagai `object-position: {x*100}% {y*100}%`.
--   zoom              : 1..3, faktor perbesaran (`transform: scale(zoom)`).
--                       1 = tanpa perbesaran, juga perilaku lama.
--
-- REAL (bukan NUMERIC): ini angka tampilan, bukan uang — presisi float sudah
-- jauh melebihi ketelitian yang bisa dilihat mata, dan lebih hemat.
--
-- Idempotent. Jalankan setelah migrasi 1–53.
-- =============================================================================

ALTER TABLE activity_photos
    ADD COLUMN IF NOT EXISTS focus_x REAL NOT NULL DEFAULT 0.5,
    ADD COLUMN IF NOT EXISTS focus_y REAL NOT NULL DEFAULT 0.5,
    ADD COLUMN IF NOT EXISTS zoom    REAL NOT NULL DEFAULT 1.0;

-- Batas nilai dijaga di database, bukan hanya di formulir. Nilai di luar rentang
-- tak akan membuat halaman gagal, tapi menghasilkan tampilan yang mustahil
-- diperbaiki lewat UI (foto melayang keluar bingkai, atau zoom 400× yang hanya
-- menampilkan satu piksel) — lebih baik ditolak di sini.
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'activity_photos_focus_range'
    ) THEN
        ALTER TABLE activity_photos
            ADD CONSTRAINT activity_photos_focus_range
            CHECK (focus_x >= 0 AND focus_x <= 1 AND focus_y >= 0 AND focus_y <= 1);
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'activity_photos_zoom_range'
    ) THEN
        ALTER TABLE activity_photos
            ADD CONSTRAINT activity_photos_zoom_range
            CHECK (zoom >= 1 AND zoom <= 3);
    END IF;
END $$;

COMMENT ON COLUMN activity_photos.focus_x IS
    'Titik fokus horizontal 0..1 (0.5 = tengah). Dipakai sebagai object-position X saat foto dipotong cover.';
COMMENT ON COLUMN activity_photos.focus_y IS
    'Titik fokus vertikal 0..1 (0.5 = tengah). Dipakai sebagai object-position Y saat foto dipotong cover.';
COMMENT ON COLUMN activity_photos.zoom IS
    'Perbesaran 1..3 (1 = apa adanya). Diterapkan sebagai transform: scale() di dalam bingkai.';
