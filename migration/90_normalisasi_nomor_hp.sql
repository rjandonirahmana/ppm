-- =============================================================================
-- 90_normalisasi_nomor_hp.sql — Satukan `users.phone_number` ke bentuk `628…`.
--
-- ── MASALAHNYA ───────────────────────────────────────────────────────────────
-- `models::phone` menetapkan SATU bentuk simpan: `628xxxxxxxxx`, karena bentuk
-- itulah yang langsung dipakai WAHA sebagai chat-id (`628xxx@c.us`). Aturan itu
-- ditegakkan di semua pintu masuk yang menulis nomor HARI INI — pendaftaran,
-- ganti nomor, sunting profil di /manajemen-user, buku tamu.
--
-- Yang TIDAK pernah lewat pintu-pintu itu: baris yang lahir sebelum aturannya
-- ada, dan 512 baris impor daftar induk (migrasi 74). Di sana nomor tersimpan
-- apa adanya — '0857…', '+62 857-…', '857…' (spreadsheet yang memperlakukan
-- nomor sebagai bilangan lalu memakan nol depannya), atau dengan spasi.
--
-- Akibatnya BUKAN galat, melainkan diam:
--   • chat-id yang terbentuk jadi '0857…@c.us' — bukan alamat siapa pun. WAHA
--     menerimanya tanpa protes dan menjawab sukses, jadi lupa-sandi, OTP, dan
--     pengingat tagihan untuk orang itu tak pernah sampai, selamanya, tanpa
--     satu pun tanda di log. (Sisi kodenya sudah ditutup: `send_wa_text` kini
--     menormalkan sendiri dan MENOLAK nomor yang tak bisa ditafsirkan. Migrasi
--     ini membereskan datanya, supaya penolakan itu tak jadi kejadian harian.)
--   • pencarian yang membandingkan teks tak menemukannya — itulah kenapa
--     `HP_COCOK_SQL` di repository harus mengadu digit dalam tiga bentuk.
--
-- ── ATURAN YANG DIPAKAI (sama persis dengan `models::normalisasi_hp`) ─────────
--   digit = buang semua non-angka
--   '620…' → sisanya      ('+62 0812…' — kode negara DAN nol daerah sekaligus)
--   '62…'  → sisanya
--   '0…'   → sisanya
--   selain itu → apa adanya
--   sah bila sisanya diawali '8' dan panjangnya 8–13 → simpan '62' || sisanya
-- Urutan '620' SEBELUM '62' menentukan; dibalik, '6208…' lolos sebagai nomor
-- yang sudah benar dan cacatnya justru diabadikan.
--
-- ── YANG SENGAJA TIDAK DISENTUH ──────────────────────────────────────────────
--   • Nomor yang tak bisa ditafsirkan (nomor rumah, potongan angka, catatan
--     seperti 'tidak punya'). Menebak isinya lebih buruk daripada
--     membiarkannya: yang salah tebak akan mengirim pesan pribadi santri ke
--     nomor orang lain. Baris ini dilaporkan lewat NOTICE untuk dibetulkan
--     manusia di /manajemen-user.
--   • Baris yang bentuk barunya BENTROK dengan nomor milik akun lain
--     (`phone_number` UNIK). Bentrok begini justru temuan penting — biasanya
--     satu orang punya dua akun, atau nomor wali dipakai di dua santri — dan
--     penyelesaiannya keputusan pengurus, bukan migrasi.
--
-- Idempoten: dijalankan ulang tak mengubah apa pun (yang sudah `628…` sudah
-- sama dengan hasil normalisasinya sendiri).
-- =============================================================================

-- Bentuk normal sebuah nomor, atau NULL bila tak bisa ditafsirkan.
-- Ditulis sebagai fungsi SEMENTARA supaya bisa dipakai tiga kali di bawah tanpa
-- menyalin ekspresi CASE yang sama — dan dibuang di akhir berkas, karena skema
-- produksi tak perlu menanggung fungsi bantu milik satu migrasi.
CREATE OR REPLACE FUNCTION pg_temp.hp_normal(mentah text)
RETURNS text
LANGUAGE sql
IMMUTABLE
AS $$
    WITH d AS (
        SELECT regexp_replace(coalesce(mentah, ''), '\D', '', 'g') AS digit
    ), i AS (
        SELECT CASE
                   WHEN digit LIKE '620%' THEN substring(digit FROM 4)
                   WHEN digit LIKE '62%'  THEN substring(digit FROM 3)
                   WHEN digit LIKE '0%'   THEN substring(digit FROM 2)
                   ELSE digit
               END AS inti
          FROM d
    )
    SELECT CASE
               WHEN inti LIKE '8%' AND length(inti) BETWEEN 8 AND 13
                   THEN '62' || inti
               ELSE NULL
           END
      FROM i;
$$;

DO $$
DECLARE
    n_ubah   int;
    n_gagal  int;
    n_bentrok int;
    r        record;
BEGIN
    -- ── 1) Perbaiki yang bisa diperbaiki ────────────────────────────────────
    -- DISTINCT ON (baru) — SATU baris saja per nomor hasil.
    --
    -- Tanpa ini migrasi bisa gagal total di kasus yang justru paling mungkin:
    -- dua akun menyimpan nomor yang SAMA dalam bentuk berbeda ('0857…' dan
    -- '+62 857-…'). Keduanya menjadi '62857…', dan `NOT EXISTS` di bawah
    -- meloloskan KEDUANYA — subquery membaca snapshot sebelum pernyataan ini,
    -- jadi tak satu pun melihat perubahan yang lain. Hasilnya pelanggaran
    -- `uq_users_phone` yang membatalkan seluruh migrasi.
    --
    -- Yang tertua (id terkecil) dibetulkan; sisanya tertinggal dan muncul di
    -- laporan bentrok pada langkah 2 — di mana ia memang termasuk.
    WITH calon AS (
        SELECT DISTINCT ON (pg_temp.hp_normal(u.phone_number))
               u.id,
               u.phone_number                   AS lama,
               pg_temp.hp_normal(u.phone_number) AS baru
          FROM users u
         WHERE u.phone_number IS NOT NULL
           AND u.phone_number <> ''
         ORDER BY pg_temp.hp_normal(u.phone_number), u.id
    )
    UPDATE users u
       SET phone_number = c.baru,
           updated_at   = NOW()
      FROM calon c
     WHERE u.id = c.id
       AND c.baru IS NOT NULL
       AND c.baru <> c.lama
       -- Penjaga bentrok. `uq_users_phone` (migrasi 19) tetap lapis terakhir;
       -- ini yang mengubah bentrok jadi baris yang DILEWATI dan DILAPORKAN,
       -- bukan migrasi yang berhenti di tengah dengan galat constraint mentah.
       AND NOT EXISTS (
               SELECT 1 FROM users x
                WHERE x.phone_number = c.baru AND x.id <> u.id
           );
    GET DIAGNOSTICS n_ubah = ROW_COUNT;
    RAISE NOTICE 'Nomor dinormalkan ke bentuk 62…: % baris', n_ubah;

    -- ── 2) Laporkan yang bentrok (belum ternormalkan, tapi BISA) ────────────
    SELECT count(*) INTO n_bentrok
      FROM users u
     WHERE u.phone_number IS NOT NULL
       AND u.phone_number <> ''
       AND pg_temp.hp_normal(u.phone_number) IS NOT NULL
       AND pg_temp.hp_normal(u.phone_number) <> u.phone_number;
    IF n_bentrok > 0 THEN
        RAISE NOTICE '% baris TAK diubah karena nomornya bentrok dengan akun lain:', n_bentrok;
        FOR r IN
            SELECT u.id, u.full_name, u.phone_number,
                   pg_temp.hp_normal(u.phone_number) AS baru
              FROM users u
             WHERE u.phone_number IS NOT NULL
               AND u.phone_number <> ''
               AND pg_temp.hp_normal(u.phone_number) IS NOT NULL
               AND pg_temp.hp_normal(u.phone_number) <> u.phone_number
             ORDER BY u.id
        LOOP
            RAISE NOTICE '  id=% "%": % → % (sudah dipakai akun lain)',
                r.id, r.full_name, r.phone_number, r.baru;
        END LOOP;
    END IF;

    -- ── 3) Laporkan yang tak bisa ditafsirkan ───────────────────────────────
    SELECT count(*) INTO n_gagal
      FROM users u
     WHERE u.phone_number IS NOT NULL
       AND u.phone_number <> ''
       AND pg_temp.hp_normal(u.phone_number) IS NULL;
    IF n_gagal > 0 THEN
        RAISE NOTICE '% baris nomornya TAK bisa ditafsirkan — betulkan manual di /manajemen-user:', n_gagal;
        FOR r IN
            SELECT u.id, u.full_name, u.phone_number
              FROM users u
             WHERE u.phone_number IS NOT NULL
               AND u.phone_number <> ''
               AND pg_temp.hp_normal(u.phone_number) IS NULL
             ORDER BY u.id
             LIMIT 50
        LOOP
            RAISE NOTICE '  id=% "%": %', r.id, r.full_name, r.phone_number;
        END LOOP;
    END IF;
END $$;

DROP FUNCTION IF EXISTS pg_temp.hp_normal(text);

-- =============================================================================
-- VERIFIKASI (jalankan sesudahnya; keduanya harus 0 selain yang dilaporkan
-- NOTICE di atas sebagai bentrok/tak tertafsirkan):
--
--   -- Nomor yang belum berbentuk 62…:
--   SELECT id, full_name, phone_number FROM users
--    WHERE phone_number IS NOT NULL AND phone_number <> ''
--      AND phone_number !~ '^628[0-9]{7,12}$'
--    ORDER BY id;
--
--   -- Nomor kembar (seharusnya mustahil — uq_users_phone):
--   SELECT phone_number, count(*) FROM users
--    WHERE phone_number IS NOT NULL GROUP BY 1 HAVING count(*) > 1;
-- =============================================================================
