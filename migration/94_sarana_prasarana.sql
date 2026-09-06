-- =============================================================================
-- 94_sarana_prasarana.sql — Pendataan sarana & prasarana pondok.
--
-- ── PERTANYAAN YANG DIJAWAB TABEL INI ────────────────────────────────────────
-- Empat, dan hanya empat: pondok ini PUNYA APA, ada BERAPA, letaknya DI MANA,
-- dan KEADAANNYA bagaimana. Semuanya pertanyaan yang hari ini dijawab dengan
-- berkeliling dan bertanya-tanya, lalu jawabannya hilang lagi.
--
-- Yang SENGAJA tidak dimuat: harga, sumber dana, nomor inventaris resmi, dan
-- riwayat pemindahan. Semuanya masuk akal untuk inventaris yang diaudit, dan
-- semuanya berubah menjadi kolom kosong pada pendataan yang diisi sambil
-- berjalan. Kalau kelak dibutuhkan, menambah kolom jauh lebih mudah daripada
-- membujuk orang mengisi kolom yang tak mereka pahami.
--
-- ── KENAPA `jumlah`, BUKAN SATU BARIS PER BENDA ──────────────────────────────
-- "Kursi santri, Aula, 120 buah" adalah cara orang pondok menyebutnya, dan 120
-- baris kursi tak menambah satu keterangan pun yang berguna. Benda yang memang
-- perlu dibedakan satu per satu (mis. laptop dengan nomor seri) cukup didata
-- sebagai barisnya sendiri berjumlah 1 — bentuk ini menampung keduanya,
-- sedangkan baris-per-benda tidak.
--
-- Idempotent. Jalankan setelah migrasi 1–93.
-- TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh` yang membungkusnya.
-- =============================================================================

CREATE TABLE IF NOT EXISTS sarana (
    id          BIGSERIAL PRIMARY KEY,
    nama        VARCHAR(120) NOT NULL,
    -- Kategori DIBATASI CHECK, bukan dibiarkan bebas: daftar bebas cepat
    -- berisi "Elektronik", "elektronik", dan "Alat elektronik" sebagai tiga
    -- hal berbeda, dan penyaringan per kategori berhenti berguna sejak hari
    -- kedua. Daftarnya sepadan dengan `models::sarana::KATEGORI`.
    kategori    VARCHAR(20) NOT NULL
                CHECK (kategori IN ('gedung','ruang','perabot','elektronik',
                                    'kendaraan','ibadah','olahraga','lainnya')),
    lokasi      VARCHAR(120) NOT NULL DEFAULT '',
    -- Positif selalu. Nol berarti barangnya tak ada, dan barang yang tak ada
    -- tak perlu barisnya sendiri — ia dihapus.
    jumlah      INTEGER NOT NULL DEFAULT 1 CHECK (jumlah > 0),
    kondisi     VARCHAR(20) NOT NULL DEFAULT 'baik'
                CHECK (kondisi IN ('baik','rusak_ringan','rusak_berat')),
    catatan     TEXT NOT NULL DEFAULT '',
    -- Siapa yang terakhir menyentuh barisnya. ON DELETE SET NULL: pendataan
    -- barang tak boleh ikut hilang ketika akun pengurusnya dihapus — barangnya
    -- masih ada di pondok.
    dicatat_oleh BIGINT REFERENCES users(id) ON DELETE SET NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Daftar dibaca terurut nama; index ini yang membuatnya index scan, bukan sort
-- atas seluruh tabel setiap kali halamannya dibuka.
CREATE INDEX IF NOT EXISTS idx_sarana_nama ON sarana (nama);

-- Dua penyaring yang benar-benar dipakai di layar. Keduanya kolom bernilai
-- SEDIKIT (delapan kategori, tiga kondisi), jadi indexnya kecil dan tetap
-- berguna: yang dicari orang justru himpunan kecilnya — "apa saja yang rusak
-- berat", bukan "semua yang baik".
CREATE INDEX IF NOT EXISTS idx_sarana_kategori ON sarana (kategori);
CREATE INDEX IF NOT EXISTS idx_sarana_rusak
    ON sarana (kondisi) WHERE kondisi <> 'baik';
