-- =============================================================================
-- 76_pengajuan_menunggu_unik.sql — Satu santri, satu pengajuan yang menunggu.
--
-- Aturannya sudah ada sejak migrasi 75, tapi hanya di dalam kode: sebelum
-- menyimpan, `service::finance::periksa_kiriman` menanyakan siapa saja yang
-- masih punya baris berstatus 'menunggu' dan menolak kiriman yang menyentuh
-- mereka. Yang tak dijaga adalah JARAK antara pertanyaan itu dan penyimpanannya
-- — dan jaraknya bukan sepersekian detik, melainkan selama unggahan foto bukti
-- ke RustFS: pemeriksaannya sengaja dijalankan LEBIH DULU supaya kiriman yang
-- pasti ditolak tak menghabiskan kuota keluarga.
--
-- Akibatnya dua request yang berangkat bersamaan — orang tua menekan "Kirim"
-- dua kali karena sinyal pondok lambat, atau ayah dan ibu mengirim bukti
-- transfer yang sama dari dua HP — sama-sama lolos pemeriksaan (keduanya
-- bertanya saat belum ada baris 'menunggu'), lalu sama-sama menyimpan. Hasilnya
-- dua baris pengajuan identik yang bisa diverifikasi DUA pengurus berbeda, dan
-- satu setoran tercatat dua kali sebagai dua periode berbeda.
--
-- Pemeriksaan di kode tidak dibuang: ia tetap yang menghasilkan kalimat yang
-- dibaca keluarga ("Masih ada pengajuan yang sedang diperiksa untuk Ahmad"),
-- lengkap dengan nama santrinya, dan tetap menolak SEBELUM foto diunggah.
-- Index ini adalah jaring pengaman terakhir untuk kasus yang tak bisa dilihat
-- kode mana pun dari satu proses: dua transaksi yang berjalan bersamaan.
--
-- Idempotent. Jalankan setelah migrasi 1–75.
-- =============================================================================

-- ── 1. Pastikan datanya sudah bersih ─────────────────────────────────────────
-- CREATE UNIQUE INDEX gagal bila sudah ada pelanggaran, dengan pesan Postgres
-- yang menyebut nama index dan sebuah key — tak satu pun memberi tahu pengurus
-- APA yang harus dilakukan. Jadi diperiksa lebih dulu, dan bila ada, migrasinya
-- berhenti dengan kalimat yang bisa ditindaklanjuti.
DO $$
DECLARE
    ganda TEXT;
BEGIN
    SELECT string_agg(nama, ', ')
      INTO ganda
      FROM (
          SELECT u.full_name || ' (' || count(*) || ' pengajuan)' AS nama
            FROM bills b
            JOIN users u ON u.id = b.user_id
           WHERE b.status = 'menunggu'
           GROUP BY u.id, u.full_name
          HAVING count(*) > 1
           ORDER BY u.full_name
      ) t;

    IF ganda IS NOT NULL THEN
        RAISE EXCEPTION
            'Masih ada santri dengan lebih dari satu pengajuan menunggu: %. '
            'Selesaikan dulu di layar Pembayaran Santri → tab "Menunggu" '
            '(setujui yang benar, tolak sisanya dengan alasan), lalu jalankan '
            'ulang migrasi ini.', ganda;
    END IF;
END $$;

-- ── 2. Index unik parsial ────────────────────────────────────────────────────
-- Parsial (`WHERE status = 'menunggu'`) karena aturannya memang hanya berlaku
-- untuk antrean: satu santri boleh — dan memang seharusnya — punya banyak baris
-- 'lunas' sepanjang tahun, dan banyak baris 'ditolak' yang sengaja disimpan
-- sebagai jejak.
--
-- Nama BARU yang tak menyerupai index mana pun yang sudah ada: `IF NOT EXISTS`
-- mencocokkan NAMA, bukan definisi (jebakan yang sudah menggigit di migrasi 45,
-- 70, dan 75).
CREATE UNIQUE INDEX IF NOT EXISTS uq_bills_pengajuan_menunggu
    ON bills (user_id) WHERE status = 'menunggu';

ANALYZE bills;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   SELECT pg_get_indexdef(indexrelid) FROM pg_index
--    WHERE indrelid = 'bills'::regclass
--      AND indexrelid::regclass::text = 'uq_bills_pengajuan_menunggu';
--   -- harus 1 baris, berisi UNIQUE ... WHERE (status = 'menunggu'::text)
--
--   -- Harus KOSONG (kalau tidak, bagian 1 di atas semestinya sudah menolak):
--   SELECT user_id, count(*) FROM bills WHERE status = 'menunggu'
--    GROUP BY user_id HAVING count(*) > 1;
-- =============================================================================
