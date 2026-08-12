-- =============================================================================
-- 81_jenjang_dasar_pra_saringan.sql — Dua jenjang KBM baru: Dasar & Pra Saringan.
--
-- Jenjang KBM sebelumnya: lambatan → cepatan → saringan → hadist_besar.
-- Sekarang: dasar → lambatan → cepatan → pra_saringan → saringan → hadist_besar.
--
-- URUTANNYA BUKAN SEKADAR TATA LETAK. `models::jenjang_berikutnya` menentukan
-- kenaikan jenjang dari POSISI di `models::JENJANG`, jadi menyisipkan
-- `pra_saringan` di antara cepatan dan saringan mengubah ke mana santri cepatan
-- naik: sekarang ke pra saringan, bukan langsung saringan. Itu memang yang
-- dimaksudkan; disebut di sini supaya perubahan perilakunya tak tersembunyi di
-- balik migrasi yang terlihat sekadar menambah dua nilai.
--
-- CHECK-nya diganti, bukan dilonggarkan jadi teks bebas: daftar tertutup inilah
-- yang membuat satu salah ketik ("saringn") ketahuan saat menyimpan, bukan
-- berbulan-bulan kemudian sebagai kelas yang tak pernah muncul di rekap mana
-- pun. Kolom Rust dan CHECK ini WAJIB sepadan — ubah keduanya bersama.
--
-- Tak ada data yang perlu dipindahkan: dua nilai ini BARU, dan kelas yang sudah
-- ada tetap sah menurut CHECK yang baru.
--
-- Idempotent. Jalankan setelah migrasi 1–80.
-- =============================================================================

ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_jenjang;
ALTER TABLE classes ADD CONSTRAINT chk_classes_jenjang CHECK (
    (category = 'kbm' AND jenjang IN (
        'dasar', 'lambatan', 'cepatan', 'pra_saringan', 'saringan', 'hadist_besar'
    ))
 OR (category <> 'kbm' AND jenjang IS NULL)
);

ANALYZE classes;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   SELECT pg_get_constraintdef(oid) FROM pg_constraint
--    WHERE conrelid = 'classes'::regclass AND conname = 'chk_classes_jenjang';
--   -- harus memuat 'dasar' dan 'pra_saringan'.
--
--   -- Sebaran jenjang yang ada sekarang:
--   SELECT COALESCE(jenjang,'(non-KBM)') AS jenjang, count(*)
--     FROM classes GROUP BY 1 ORDER BY 1;
--
--   -- Coba nilai ngawur — HARUS ditolak CHECK:
--   -- UPDATE classes SET jenjang = 'saringn' WHERE category = 'kbm';
-- =============================================================================
