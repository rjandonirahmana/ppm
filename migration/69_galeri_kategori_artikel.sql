-- =============================================================================
-- 69_galeri_kategori_artikel.sql — Galeri berkategori (+video) & artikel.
--
-- BAGIAN 1 — GALERI PUNYA KATEGORI & JENIS MEDIA
-- `activity_photos` selama ini satu tumpukan foto tanpa penanda apa pun, jadi
-- halaman publik hanya bisa menampilkannya sebagai satu grid. Padahal isinya
-- tiga hal berbeda yang tampil di tempat berbeda:
--
--   video_utama → SATU video yang berjalan di kepala halaman depan
--   kegiatan    → foto kegiatan santri (grid "Kegiatan")
--   fasilitas   → foto sarana pondok (grid "Fasilitas")
--
-- `media_type` dipisah dari kategori: video utama hari ini berupa video, tapi
-- pondok yang belum punya rekaman bisa memakai foto sebagai penggantinya —
-- dan sebaliknya, kelak bisa ada video kegiatan. Menyimpulkan jenis media dari
-- ekstensi URL bekerja sampai satu tautan tak berekstensi muncul.
--
-- `caption` sudah ada sejak awal; tak diapa-apakan.
--
-- BAGIAN 2 — ARTIKEL
-- Halaman depan menampilkan artikel yang dikelola admin. Dibuat sesederhana
-- mungkin: judul, ringkasan, isi, gambar sampul, dan penanda terbit. Tanpa
-- kategori, tag, atau komentar — semuanya bisa ditambahkan kalau memang
-- dibutuhkan, dan menebaknya sekarang hanya melahirkan kolom yang tak terisi.
--
-- `slug` unik dipakai sebagai alamat publik (/artikel/<slug>) supaya tautannya
-- bisa dibagikan dan tetap bermakna, bukan /artikel/47.
--
-- Idempotent. Jalankan setelah migrasi 1–68.
-- =============================================================================

ALTER TABLE activity_photos ADD COLUMN IF NOT EXISTS category VARCHAR(20);
ALTER TABLE activity_photos ADD COLUMN IF NOT EXISTS media_type VARCHAR(10);

-- Baris lama = foto kegiatan: itu satu-satunya yang pernah diunggah lewat UI
-- galeri sebelum kategori ada.
UPDATE activity_photos SET category = 'kegiatan' WHERE category IS NULL;
UPDATE activity_photos SET media_type = 'image' WHERE media_type IS NULL;

ALTER TABLE activity_photos ALTER COLUMN category SET DEFAULT 'kegiatan';
ALTER TABLE activity_photos ALTER COLUMN category SET NOT NULL;
ALTER TABLE activity_photos ALTER COLUMN media_type SET DEFAULT 'image';
ALTER TABLE activity_photos ALTER COLUMN media_type SET NOT NULL;

ALTER TABLE activity_photos DROP CONSTRAINT IF EXISTS chk_photos_category;
ALTER TABLE activity_photos ADD CONSTRAINT chk_photos_category
    CHECK (category IN ('video_utama', 'kegiatan', 'fasilitas'));

ALTER TABLE activity_photos DROP CONSTRAINT IF EXISTS chk_photos_media;
ALTER TABLE activity_photos ADD CONSTRAINT chk_photos_media
    CHECK (media_type IN ('image', 'video'));

-- Halaman depan membaca per-kategori, terurut.
CREATE INDEX IF NOT EXISTS idx_photos_kategori
    ON activity_photos (category, sort_order, id);

-- =============================================================================

CREATE TABLE IF NOT EXISTS articles (
    id          BIGSERIAL PRIMARY KEY,
    slug        VARCHAR(160) NOT NULL UNIQUE,
    title       VARCHAR(200) NOT NULL,
    -- Ringkasan untuk kartu di halaman depan.
    excerpt     TEXT NOT NULL DEFAULT '',
    body        TEXT NOT NULL DEFAULT '',
    cover_url   TEXT,
    -- Draf tak tampil di halaman publik. Default FALSE supaya artikel yang
    -- baru dibuat tak langsung terbit sebelum sempat dibaca ulang.
    published   BOOLEAN NOT NULL DEFAULT FALSE,
    created_by  BIGINT REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Daftar publik: hanya yang terbit, terbaru dulu. Index parsial — draf tak
-- pernah ikut dibaca, jadi tak perlu ikut diindeks.
CREATE INDEX IF NOT EXISTS idx_articles_published
    ON articles (created_at DESC) WHERE published;

ANALYZE activity_photos;

-- Verifikasi:
--   SELECT category, media_type, count(*) FROM activity_photos GROUP BY 1, 2;
--   SELECT slug, title, published FROM articles ORDER BY created_at DESC;
