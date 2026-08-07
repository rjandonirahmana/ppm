-- =============================================================================
-- 65_kbm_dan_jenjang.sql — Kelas jadi TIGA jenis; jenjang jadi kolom sendiri.
--
-- KEADAAN SEBELUMNYA
-- `classes.category` teks bebas, dan di produksi berisi enam nilai yang
-- mencampur dua pertanyaan berbeda:
--     Lambatan, cepatan, hadis besar   → sebenarnya JENJANG kelas KBM
--     non kbm, piket, Sholat wajib     → sebenarnya JENIS kegiatan
-- Sementara `classes.golongan` (migrasi 16, 'Bacaan'/'Makna') berisi 'makna',
-- 'piket', dan kosong — sumbu yang sudah tak dipakai lagi sebagaimana
-- dirancang dulu.
--
-- Akibatnya tak ada satu pun kolom yang bisa menjawab "kelas ini KBM atau
-- bukan?" tanpa mencocokkan daftar kata. Dua gerbang penting sudah terlanjur
-- salah karenanya:
--   • `category_allows_recording` menuntut kategori PERSIS "pengajian" — tak
--     ada satu pun kelas yang begitu, jadi siaran suara mati di semua kelas;
--   • pemilihan "kelas akademik" santri (repository::kelas_utama_lateral)
--     terpaksa menebak lewat golongan IN ('bacaan','makna').
--
-- YANG DILAKUKAN
--   1. `jenjang` — kolom baru: lambatan → cepatan → saringan → hadist_besar
--      (berurutan; santri naik jenjang setelah kurikulumnya tuntas). Hanya
--      untuk kelas KBM.
--   2. `category` dinormalkan jadi TIGA nilai: 'kbm' | 'non_kbm' | 'bacaan'.
--      Bacaan Al-Quran berdiri sendiri — ia bukan KBM (tak berjenjang, tak
--      terikat aturan satu-kelas) tapi juga bukan kegiatan non-KBM seperti
--      piket atau apel. Piket, apel, sholat, totalan → non_kbm.
--   3. Trigger: satu santri paling banyak SATU kelas KBM. Kelas non-KBM tetap
--      bebas berapa pun (piket + apel + sholat sekaligus itu normal).
--   4. WALI KELAS hanya ada di kelas KBM. Kelas Bacaan dan non-KBM cukup punya
--      PAMONG — tugasnya menunjuk guru tiap sesi dan (bila kelasnya memakai
--      verifikasi dua langkah) menyetujui absensi.
--
--      Ini yang membuat perizinan sederhana: karena santri hanya punya satu
--      kelas KBM, ia hanya perlu SATU izin ke satu wali kelas. Selama wali
--      kelas bisa menempel di kelas mana pun, satu izin sehari bisa pecah ke
--      tiga-empat penyetuju berbeda yang semuanya harus menekan tombol.
--
-- KENAPA `golongan` TIDAK DIBUANG DI SINI
-- Migrasi ini dijalankan SEBELUM aplikasi versi barunya menyala. Aplikasi yang
-- sedang berjalan masih menyeleksi kolom `golongan`; membuangnya sekarang
-- membuat seluruh halaman kelas galat sampai deploy selesai. Kolomnya dibiarkan
-- utuh dan dibuang di migrasi terpisah setelah versi baru jalan.
--
-- Idempotent. Jalankan setelah migrasi 1–64.
-- =============================================================================

ALTER TABLE classes ADD COLUMN IF NOT EXISTS jenjang VARCHAR(30);

-- 1) Jenjang diturunkan dari kategori LAMA, sebelum kategorinya ditimpa.
--    Hanya kelas yang kategorinya memang nama jenjang yang terisi.
UPDATE classes
   SET jenjang = CASE lower(btrim(coalesce(category, '')))
                     WHEN 'lambatan'     THEN 'lambatan'
                     WHEN 'cepatan'      THEN 'cepatan'
                     WHEN 'saringan'     THEN 'saringan'
                     WHEN 'hadis besar'  THEN 'hadist_besar'
                     WHEN 'hadist besar' THEN 'hadist_besar'
                     ELSE NULL
                 END
 WHERE jenjang IS NULL;

-- 2) Kategori: kelas berjenjang = KBM; yang kategorinya menyebut "bacaan"
--    = bacaan; sisanya non-KBM.
UPDATE classes
   SET category = CASE
                    WHEN jenjang IS NOT NULL                            THEN 'kbm'
                    WHEN lower(coalesce(category, '')) LIKE '%bacaan%'  THEN 'bacaan'
                    ELSE 'non_kbm'
                  END
 WHERE category IS NULL OR category NOT IN ('kbm', 'non_kbm', 'bacaan');

ALTER TABLE classes ALTER COLUMN category SET DEFAULT 'non_kbm';
UPDATE classes SET category = 'non_kbm' WHERE category IS NULL;
ALTER TABLE classes ALTER COLUMN category SET NOT NULL;

ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_category;
ALTER TABLE classes ADD CONSTRAINT chk_classes_category
    CHECK (category IN ('kbm', 'non_kbm', 'bacaan'));

-- Jenjang hanya bermakna untuk KBM — dan setiap KBM wajib punya. Tanpa syarat
-- kedua ini, "naik jenjang" tak punya titik berangkat.
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_jenjang;
ALTER TABLE classes ADD CONSTRAINT chk_classes_jenjang CHECK (
    (category = 'kbm' AND jenjang IN ('lambatan', 'cepatan', 'saringan', 'hadist_besar'))
 OR (category <> 'kbm' AND jenjang IS NULL)
);

CREATE INDEX IF NOT EXISTS idx_classes_kbm ON classes (jenjang) WHERE category = 'kbm';

-- 4) Wali kelas hanya di KBM. Yang terlanjur menempel di kelas lain dilepas —
--    di produksi ada dua (Sholat, apel). Pamongnya TIDAK diutak-atik: justru
--    pamong-lah petugas kelas non-KBM.
UPDATE classes SET wali_kelas_id = NULL WHERE category <> 'kbm' AND wali_kelas_id IS NOT NULL;

ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_wali_kbm;
ALTER TABLE classes ADD CONSTRAINT chk_classes_wali_kbm
    CHECK (wali_kelas_id IS NULL OR category = 'kbm');

-- 3) Satu santri = satu kelas KBM.
--
-- Trigger, bukan UNIQUE INDEX: syaratnya bergantung pada `classes.category` di
-- tabel LAIN, dan index parsial Postgres tak boleh menengok ke sana. Diperiksa
-- di database, bukan cuma di service, karena inilah aturan yang menentukan
-- "wali kelas siapa" untuk perizinan, rapor, dan kenaikan jenjang — satu jalur
-- tulis yang lupa memeriksanya sudah cukup merusaknya.
CREATE OR REPLACE FUNCTION cek_satu_kelas_kbm() RETURNS trigger AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM classes c WHERE c.id = NEW.class_id AND c.category = 'kbm')
       AND EXISTS (
            SELECT 1 FROM class_participants cp
              JOIN classes c2 ON c2.id = cp.class_id
             WHERE cp.user_id = NEW.user_id
               AND c2.category = 'kbm'
               AND cp.class_id <> NEW.class_id
       )
    THEN
        RAISE EXCEPTION 'santri sudah terdaftar di kelas KBM lain'
            USING ERRCODE = 'unique_violation';
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_satu_kelas_kbm ON class_participants;
CREATE TRIGGER trg_satu_kelas_kbm
    BEFORE INSERT OR UPDATE OF class_id, user_id ON class_participants
    FOR EACH ROW EXECUTE FUNCTION cek_satu_kelas_kbm();

-- Verifikasi:
--   SELECT id, name, category, jenjang FROM classes ORDER BY category, jenjang, id;
--   SELECT cp.user_id, count(*) FROM class_participants cp
--     JOIN classes c ON c.id = cp.class_id WHERE c.category = 'kbm'
--    GROUP BY cp.user_id HAVING count(*) > 1;   -- harus kosong
