-- =============================================================================
-- 72_saldo_awal_masuk_buku_besar.sql — Jadikan `users.points` benar-benar sama
-- dengan `SUM(point_logs.delta)`.
--
-- ── MASALAHNYA ───────────────────────────────────────────────────────────────
-- Migrasi 28 menyetel `ALTER TABLE users ALTER COLUMN points SET DEFAULT 300`.
-- Sejak itu setiap santri baru lahir dengan saldo 300 LANGSUNG dari default
-- kolom — tanpa satu pun baris di `point_logs`. Saldo awalnya tak pernah masuk
-- buku besar.
--
-- Dua akibat yang selama ini tak terlihat:
--
--   1. Riwayat poin seorang santri tak bisa menjelaskan saldonya sendiri.
--      Contoh nyata dari produksi: santri dengan saldo 210 sementara seluruh
--      lognya berjumlah −90. Tak ada baris mana pun yang menerangkan selisih
--      300 itu. Bagi yang membacanya, saldonya seolah muncul entah dari mana.
--
--   2. Invarian yang dijaga trigger migrasi 32 — `points = ΣΔ` — TIDAK PERNAH
--      benar untuk siapa pun. Begitu ada pemeriksaan rekonsiliasi, ia melapor
--      SETIAP santri menyimpang. Pemeriksaan yang selalu berteriak akan
--      dimatikan orang, dan sesudah itu penyimpangan yang sungguhan pun lewat
--      tanpa ada yang tahu.
--
-- ── KENAPA TAK BISA DITAMBAL DENGAN MENYISIPKAN LOG ──────────────────────────
-- Ini bagian yang mengecoh. Menyisipkan baris penyeimbang TIDAK menutup
-- selisih: trigger `trg_point_logs_balance` menaikkan `users.points` sebesar
-- delta yang sama, jadi kedua sisi bergerak bersamaan dan selisih
-- (points − ΣΔ) sama sekali tak berubah. Berapa pun yang disisipkan.
--
--     sebelum : points = P,      ΣΔ = S,      selisih = P − S
--     sisip D : points = P + D,  ΣΔ = S + D,  selisih = P − S   ← tetap
--
-- Satu-satunya cara menutupnya adalah menulis barisnya TANPA menjalankan
-- trigger. Karena itu trigger dimatikan sesaat di dalam transaksi ini — bukan
-- karena malas, tapi karena memang tak ada jalan lain.
--
-- ── YANG DILAKUKAN ───────────────────────────────────────────────────────────
--   1. Satu baris `point_logs` per user sebesar (points − ΣΔ), ditulis dengan
--      trigger mati → `users.points` TIDAK berubah sedikit pun, sementara ΣΔ
--      naik sampai persis sama dengannya.
--   2. Default kolom dikembalikan ke 0. Saldo awal santri baru kini dicatat
--      `repository::insert_registered_user` sebagai baris log sungguhan.
--
-- ⚠️ INI MENYERAP PENYIMPANGAN YANG SUNGGUHAN JUGA.
-- Bagi santri yang selisihnya bukan sekadar 300 (di produksi terlihat dua:
-- +515 dan +455, yaitu 300 ditambah 215 dan 155), kelebihan itu ikut masuk ke
-- baris "Saldo awal" dan setelah ini TAK BISA dibedakan lagi dari saldo awal
-- yang sah. Karena itu tiap baris di-RAISE NOTICE — jangan jalankan migrasi ini
-- sambil memalingkan muka dari keluarannya.
--
-- JALANKAN DULU INI, CATAT HASILNYA, baru migrasinya:
--
--   SELECT u.id, u.full_name, u.points AS saldo,
--          COALESCE(SUM(pl.delta), 0) AS jumlah_log,
--          u.points - COALESCE(SUM(pl.delta), 0) AS selisih,
--          u.points - COALESCE(SUM(pl.delta), 0) - 300 AS di_luar_saldo_awal
--     FROM users u
--     LEFT JOIN point_logs pl ON pl.user_id = u.id
--    WHERE u.role IN ('santri', 'santri_finance')
--    GROUP BY u.id, u.full_name, u.points
--   HAVING u.points <> COALESCE(SUM(pl.delta), 0)
--    ORDER BY abs(u.points - COALESCE(SUM(pl.delta), 0) - 300) DESC;
--
-- Kolom terakhir yang BUKAN nol adalah penyimpangan sesungguhnya. Kalau
-- angkanya penting bagi Anda (mis. poin santri pernah diubah tangan lewat SQL),
-- selesaikan dulu di sana — sesudah migrasi ini jejaknya hilang.
--
-- Butuh hak pemilik tabel `point_logs` (ALTER TABLE ... DISABLE TRIGGER).
-- Idempotent: dijalankan ulang tak menemukan selisih apa pun lagi.
-- TIDAK memuat BEGIN/COMMIT sendiri: `scripts/migrate.sh` sudah membungkus tiap
-- migrasi dalam SATU transaksi bersama pencatatannya di `schema_migrations`.
-- Menutup transaksi dari dalam berkas membuat pencatatan itu jatuh di luarnya —
-- dan "migrasi jalan tapi tak tercatat" persis kegagalan yang melahirkan skrip
-- tersebut (lihat kepala scripts/migrate.sh). Kalau dijalankan manual dengan
-- psql, bungkus sendiri: psql -1 -f berkas.sql
--
-- Jalankan setelah migrasi 1–71.
-- =============================================================================

ALTER TABLE point_logs DISABLE TRIGGER trg_point_logs_balance;

DO $$
DECLARE
    r          record;
    n          int := 0;
    total_beda bigint := 0;
BEGIN
    FOR r IN
        SELECT u.id, u.full_name, u.points,
               COALESCE(SUM(pl.delta), 0) AS jumlah_log,
               u.points - COALESCE(SUM(pl.delta), 0) AS selisih
          FROM users u
          LEFT JOIN point_logs pl ON pl.user_id = u.id
         GROUP BY u.id, u.full_name, u.points
        HAVING u.points <> COALESCE(SUM(pl.delta), 0)
         ORDER BY u.id
    LOOP
        -- Keterangan MEMBEDAKAN yang wajar dari yang tidak, selama masih bisa
        -- dibedakan. Sesudah baris ini tertulis, keduanya sama-sama jadi
        -- "saldo awal" — jadi perbedaannya diabadikan di teksnya.
        INSERT INTO point_logs (user_id, delta, reason, category)
        VALUES (
            r.id,
            r.selisih,
            CASE WHEN r.selisih = 300
                 THEN 'Saldo awal santri (rekonsiliasi migrasi 72)'
                 ELSE 'Saldo awal + selisih tak tercatat (rekonsiliasi migrasi 72)'
            END,
            'other'
        );
        n := n + 1;
        total_beda := total_beda + r.selisih;
        RAISE NOTICE 'user % (%): saldo % , log % → dicatat saldo awal %',
            r.id, r.full_name, r.points, r.jumlah_log, r.selisih;
    END LOOP;
    RAISE NOTICE '--- % user direkonsiliasi, total % poin dicatat ---', n, total_beda;
END $$;

ALTER TABLE point_logs ENABLE TRIGGER trg_point_logs_balance;

-- Saldo awal santri baru kini datang dari BARIS LOG (lihat
-- repository::insert_registered_user), bukan dari default kolom. Membiarkan
-- default 300 di sini akan membuat setiap user baru langsung menyimpang lagi
-- sebesar 300 — persis keadaan yang baru saja dibereskan.
ALTER TABLE users ALTER COLUMN points SET DEFAULT 0;

-- Jaring pengaman: kalau masih ada yang meleset, batalkan seluruhnya.
DO $$
DECLARE sisa int;
BEGIN
    SELECT count(*) INTO sisa FROM (
        SELECT u.id FROM users u
          LEFT JOIN point_logs pl ON pl.user_id = u.id
         GROUP BY u.id, u.points
        HAVING u.points <> COALESCE(SUM(pl.delta), 0)
    ) x;
    IF sisa > 0 THEN
        RAISE EXCEPTION 'Masih ada % user yang saldonya tak sama dengan jumlah lognya — dibatalkan.', sisa;
    END IF;
END $$;

-- Verifikasi (harus 0 baris):
--   SELECT u.id FROM users u
--     LEFT JOIN point_logs pl ON pl.user_id = u.id
--    GROUP BY u.id, u.points
--   HAVING u.points <> COALESCE(SUM(pl.delta), 0);
--
-- Default kolom harus 0:
--   SELECT column_default FROM information_schema.columns
--    WHERE table_name = 'users' AND column_name = 'points';
