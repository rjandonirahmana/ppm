-- =============================================================================
-- 73_status_mubalegh_pendidikan.sql — Status kemubalighan & pendidikan santri.
--
-- Dua kolom ini datang dari MASTER NIS (berkas Excel pondok), yang sudah lama
-- mencatatnya di luar aplikasi. Sebelum ini tak ada tempatnya di `users`, jadi
-- keterangannya tak pernah ikut terbawa saat santri diimpor.
--
-- ── KENAPA BUKAN BOOLEAN ─────────────────────────────────────────────────────
-- Nama kolomnya di Excel ("Mubalegh", "Sarjana") terbaca seperti pertanyaan
-- ya/tidak, dan itulah jebakannya. Nilai sebenarnya di data:
--
--   Mubalegh : Tidak 301 · Iya 131 · "Sudah MT" 18 · Belum 4 · kosong 58
--   Sarjana  : Iya 308 · Tidak 16 · "Kuliah" 13 · kosong 175
--
-- "Sudah MT" (Mubaligh Tugasan) bukan "iya" yang lebih tegas — itu keadaan
-- KETIGA: sudah mubaligh DAN sedang bertugas. Begitu pula "Kuliah": bukan
-- sarjana, bukan pula "tidak" — sedang menempuhnya. BOOLEAN memaksa keduanya
-- dibulatkan ke true/false, dan 31 santri kehilangan keterangan yang justru
-- membedakan mereka. Kolom yang membuang informasi tak bisa dikembalikan
-- belakangan tanpa membuka berkas Excel lagi.
--
-- ── KENAPA NULLABLE, DAN KENAPA BUKAN 'belum' ────────────────────────────────
-- 58 dan 175 santri kosong. Kosong di sini berarti BELUM TERCATAT, bukan
-- "tidak". Memetakannya ke 'belum' akan mengarang fakta tentang 175 orang —
-- dan sesudahnya tak ada cara membedakan mana yang benar-benar belum sarjana
-- dari mana yang datanya memang tak pernah diisi. NULL menyimpan perbedaan itu.
--
-- Kodenya huruf kecil tanpa spasi supaya query tak perlu peduli ejaan; labelnya
-- untuk layar disusun di sisi aplikasi, sebagaimana kategori lain.
--
-- Idempotent. Jalankan setelah migrasi 1–72.
-- =============================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS mubalegh_status   VARCHAR(20);
ALTER TABLE users ADD COLUMN IF NOT EXISTS pendidikan_status VARCHAR(20);

-- CHECK mengizinkan NULL secara eksplisit: tanpa `IS NULL`, baris tak terisi
-- justru lolos (CHECK yang bernilai NULL dianggap lulus) — benar, tapi
-- menuliskannya membuat maksudnya terbaca alih-alih ditebak.
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_mubalegh;
ALTER TABLE users ADD CONSTRAINT chk_users_mubalegh
    CHECK (mubalegh_status IS NULL OR mubalegh_status IN ('belum', 'iya', 'tugasan'));

ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_pendidikan;
ALTER TABLE users ADD CONSTRAINT chk_users_pendidikan
    CHECK (pendidikan_status IS NULL OR pendidikan_status IN ('belum', 'kuliah', 'sarjana'));

-- Index PARSIAL. Yang pernah ditanyakan cuma "siapa yang sudah mubaligh" dan
-- "siapa yang sudah sarjana" — bukan daftar yang kosong. Dengan menyaring NULL,
-- indexnya tak ikut menyimpan 175 baris yang tak pernah dicari.
CREATE INDEX IF NOT EXISTS idx_users_mubalegh
    ON users (mubalegh_status) WHERE mubalegh_status IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_users_pendidikan
    ON users (pendidikan_status) WHERE pendidikan_status IS NOT NULL;

-- Pemetaan dari ejaan di Excel (dilakukan skrip impor, dicatat di sini supaya
-- ada satu tempat yang menjelaskan asal nilainya):
--
--   Mubalegh  "Tidak" | "Belum"        → 'belum'
--             "Iya"   | "iya"          → 'iya'      (ejaan campur di sumber)
--             "Sudah MT"               → 'tugasan'
--             (kosong)                 → NULL
--
--   Sarjana   "Tidak"                  → 'belum'
--             "Kuliah"                 → 'kuliah'
--             "Iya"                    → 'sarjana'
--             (kosong)                 → NULL
--
-- Verifikasi:
--   SELECT mubalegh_status, count(*) FROM users GROUP BY 1 ORDER BY 2 DESC;
--   SELECT pendidikan_status, count(*) FROM users GROUP BY 1 ORDER BY 2 DESC;
