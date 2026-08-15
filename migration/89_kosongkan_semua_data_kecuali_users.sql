-- =============================================================================
-- 89_kosongkan_semua_data_kecuali_users.sql — Kosongkan SELURUH isi database
-- kecuali tabel `users`. Tabelnya sendiri, kolom, index, constraint, trigger,
-- dan fungsi TIDAK disentuh sama sekali — hanya BARISNYA yang dibuang.
--
-- ⚠️ MIGRASI INI MENGHAPUS DATA DAN TAK BISA DIBATALKAN. Sekali `migrate.sh up`
-- jalan, ia jalan di SETIAP database yang belum menerapkannya — TERMASUK
-- PRODUKSI. Ambil dump dulu:
--
--     pg_dump "$DATABASE_URL" -Fc -f sebelum_89.dump
--
-- ── APA YANG DIKOSONGKAN ─────────────────────────────────────────────────────
-- SEMUA tabel biasa di schema `public`, kecuali dua:
--   * `users`             — diminta tetap utuh.
--   * `schema_migrations` — pelacak migrasi milik scripts/migrate.sh. Kalau ini
--                           ikut kosong, seluruh migrasi 1–89 dianggap belum
--                           pernah jalan dan `migrate.sh up` berikutnya akan
--                           mengulanginya dari awal di atas skema yang sudah
--                           jadi.
--
-- Daftarnya TIDAK ditulis tangan, melainkan dibaca dari katalog saat migrasi
-- berjalan. Jadi tabel yang lahir sesudah berkas ini ditulis ikut terkosongkan,
-- dan tabel yang sudah dibuang (sessions, complaints, academic_terms — migrasi
-- 67; permit_request_classes — migrasi 86) tak membuatnya galat.
--
-- Yang ikut hilang dan mungkin tak terduga:
--   * `classes`, `class_schedules`, `class_participants` — seluruh struktur
--     kelas, jadwal, dan keanggotaan. Harus dibangun ulang dari nol.
--   * `parent_connections` — tautan orang tua ↔ santri. Barisnya ada di tabel
--     TERSENDIRI, jadi ikut terbuang meski kedua orangnya tetap ada di `users`.
--   * `rfid_devices` — semua alat scanner harus didaftarkan ulang, dan api_key
--     baru dipasang ke firmware tiap alat (lihat arduino-ppm).
--   * `app_settings`, `books`, `curriculum`, `materials`, `academic_semesters`
--     — data master/acuan.
--
-- ── KENAPA TANPA CASCADE ─────────────────────────────────────────────────────
-- `TRUNCATE ... CASCADE` ikut mengosongkan tabel yang MENUNJUK ke tabel yang
-- dikosongkan. Karena `users` satu-satunya yang dikecualikan, CASCADE justru
-- jadi satu-satunya cara `users` bisa ikut terhapus tanpa disengaja — persis
-- yang harus dicegah. Tanpa CASCADE, andai suatu hari `users` punya foreign key
-- keluar, PostgreSQL menolak seluruh TRUNCATE dan migrasi batal. Pemeriksaan di
-- BAGIAN 1 melakukan hal yang sama lebih awal dengan pesan yang bisa dibaca.
--
-- ── KENAPA `users.points` DIRESET ────────────────────────────────────────────
-- Migrasi 32 memasang trigger `trg_point_logs_balance`: `users.points` adalah
-- CACHE dari jumlah `point_logs.delta`, dan migrasi 72 menutupnya dengan jaring
-- pengaman yang MEMBATALKAN migrasi bila keduanya meleset. TRUNCATE tidak
-- membangunkan trigger per-baris, jadi mengosongkan `point_logs` saja akan
-- meninggalkan saldo lama tanpa satu pun log pendukungnya — invarian itu
-- langsung rusak. Kolom cache lain (`gate_status`, `gate_at` dari `gate_logs`;
-- `bill_reminded_at` dari `bills`) dikembalikan ke nilai awal karena alasan yang
-- sama. Baris `users`-nya sendiri, beserta identitas, sandi, peran, dan kartu
-- RFID-nya, tetap utuh.
--
-- Idempotent (menjalankan ulang di database yang sudah kosong tak berefek).
-- Jalankan setelah migrasi 1–88.
-- =============================================================================

-- ── BAGIAN 1. Pagar: `users` tak boleh bisa ikut terseret ────────────────────
DO $$
DECLARE
    fk record;
BEGIN
    FOR fk IN
        SELECT conname, confrelid::regclass AS menunjuk_ke
          FROM pg_constraint
         WHERE conrelid = 'users'::regclass
           AND contype  = 'f'
           AND confrelid <> 'users'::regclass
    LOOP
        RAISE EXCEPTION
            'users punya foreign key % ke % — mengosongkan % bisa menyeret users. Migrasi dibatalkan; putuskan dulu bagaimana kolom itu diperlakukan.',
            fk.conname, fk.menunjuk_ke, fk.menunjuk_ke;
    END LOOP;
END $$;

-- ── BAGIAN 2. Kosongkan semuanya, sekali jalan ───────────────────────────────
-- Satu perintah TRUNCATE untuk semua tabel: tak ada urutan yang perlu dipikir
-- (foreign key antar-tabel di dalam daftar sama-sama dikosongkan), dan tak ada
-- keadaan setengah jadi. RESTART IDENTITY mengembalikan sequence ke 1 supaya
-- data baru mulai dari id 1 lagi — sequence `users` tidak termasuk.
DO $$
DECLARE
    daftar text;
    jumlah int;
BEGIN
    SELECT string_agg(format('%I.%I', n.nspname, c.relname), ', ' ORDER BY c.relname),
           count(*)
      INTO daftar, jumlah
      FROM pg_class c
      JOIN pg_namespace n ON n.oid = c.relnamespace
     WHERE n.nspname = 'public'
       AND c.relkind = 'r'            -- tabel biasa; view/matview/sequence dilewati
       AND NOT c.relispartition       -- partisi ikut lewat induknya
       AND c.relname NOT IN ('users', 'schema_migrations');

    IF daftar IS NULL THEN
        RAISE NOTICE 'Tak ada tabel yang perlu dikosongkan.';
        RETURN;
    END IF;

    RAISE NOTICE 'Mengosongkan % tabel: %', jumlah, daftar;
    EXECUTE 'TRUNCATE TABLE ' || daftar || ' RESTART IDENTITY';
END $$;

-- ── BAGIAN 3. Kembalikan kolom cache di `users` ke keadaan awal ──────────────
-- Ditulis bersyarat: kolomnya boleh saja sudah dibuang migrasi lain di masa
-- depan, dan itu tak boleh menggagalkan penghapusan data yang sudah terjadi.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
                WHERE table_schema = 'public' AND table_name = 'users'
                  AND column_name = 'points') THEN
        UPDATE users SET points = 0 WHERE points <> 0;
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.columns
                WHERE table_schema = 'public' AND table_name = 'users'
                  AND column_name = 'gate_status') THEN
        UPDATE users SET gate_status = 'in' WHERE gate_status <> 'in';
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.columns
                WHERE table_schema = 'public' AND table_name = 'users'
                  AND column_name = 'gate_at') THEN
        UPDATE users SET gate_at = NULL WHERE gate_at IS NOT NULL;
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.columns
                WHERE table_schema = 'public' AND table_name = 'users'
                  AND column_name = 'bill_reminded_at') THEN
        UPDATE users SET bill_reminded_at = NULL WHERE bill_reminded_at IS NOT NULL;
    END IF;
END $$;

-- ── BAGIAN 4. Jaring pengaman: `users` harus masih berisi ────────────────────
-- Kalau baris users habis, sesuatu yang tak diduga terjadi (CASCADE dari arah
-- lain, trigger, rule). Lebih baik seluruh transaksi dibatalkan daripada
-- database ditinggalkan tanpa satu pun akun untuk masuk.
DO $$
DECLARE
    n bigint;
BEGIN
    SELECT count(*) INTO n FROM users;
    IF n = 0 THEN
        RAISE EXCEPTION 'users ikut kosong — migrasi dibatalkan seluruhnya.';
    END IF;
    RAISE NOTICE 'users tetap utuh: % baris.', n;
END $$;

ANALYZE;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   -- Semua tabel selain users & schema_migrations harus 0 baris:
--   SELECT relname, n_live_tup FROM pg_stat_user_tables
--    WHERE relname NOT IN ('users', 'schema_migrations') AND n_live_tup > 0
--    ORDER BY n_live_tup DESC;
--   -- (angka pg_stat adalah perkiraan; kalau ada yang muncul, hitung pasti
--   --  dengan SELECT count(*) FROM <tabel>.)
--
--   -- users utuh, saldo poin sudah nol semua:
--   SELECT count(*) AS jumlah_user,
--          count(*) FILTER (WHERE points <> 0) AS saldo_belum_nol,
--          count(*) FILTER (WHERE role = 'admin') AS admin
--     FROM users;
--
--   -- Invarian migrasi 72 (saldo = jumlah log) harus 0 baris:
--   SELECT u.id FROM users u LEFT JOIN point_logs pl ON pl.user_id = u.id
--    GROUP BY u.id, u.points HAVING u.points <> COALESCE(SUM(pl.delta), 0);
--
-- ── SESUDAHNYA, SEBELUM DIPAKAI LAGI ─────────────────────────────────────────
--   1. Daftarkan ulang alat RFID (rfid_devices) dan tanam api_key barunya ke
--      tiap firmware — tanpa itu semua scan ditolak.
--   2. Buat ulang kelas, jadwal, wali kelas, dan keanggotaan santri.
--   3. Sambungkan ulang orang tua ↔ santri (parent_connections).
-- =============================================================================
