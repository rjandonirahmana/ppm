-- =============================================================================
-- 86_izin_rentang_waktu_hapus_pamong.sql — Tiga pembersihan sekaligus, karena
-- ketiganya menyentuh query yang sama dan memisahkannya berarti menulis ulang
-- query itu dua kali:
--
--   1. IZIN JADI RENTANG WAKTU. `start_date + start_time` digabung jadi SATU
--      kolom `start_time TIMESTAMP`; `end_date + end_time` jadi `end_time`.
--   2. SELURUH KOLOM PAMONG DIBUANG dari empat tabel (perannya sudah dihapus
--      migrasi 84).
--   3. `permit_request_classes` DIBUANG — lihat alasannya di bagian 3.
--
-- ── 1) KENAPA DIGABUNG ───────────────────────────────────────────────────────
-- Sebuah izin adalah SATU rentang: keluar pukul sekian tanggal sekian, kembali
-- pukul sekian tanggal sekian. Menyimpannya sebagai dua tanggal + dua jam
-- memaksa setiap pembaca merakit ulang maknanya, dan sampai Ags 2026 empat
-- tempat merakitnya BERBEDA — `start_time`/`end_time` diperlakukan sebagai "jam
-- berlakunya izin pada setiap hari", sehingga santri yang pulang Jumat 14:00 dan
-- kembali Minggu 08:00 tetap dihitung ALFA sepanjang Sabtu.
--
-- Dengan satu rentang, pertanyaan "kelas ini terlewat atau tidak" punya SATU
-- jawaban: apakah jam kelasnya beririsan dengan rentang itu.
--
-- Izin SEHARI PENUH (dulu jam NULL) kini tersimpan sebagai 00:00 → 23:59:59
-- pada tanggal yang sama. Bentuknya berubah, artinya sama persis: seluruh kelas
-- hari itu beririsan dengan rentang tersebut.
--
-- Nama kolomnya SENGAJA tetap `start_time`/`end_time` — bukan diganti jadi
-- `mulai`/`selesai`. Yang berubah tipenya, bukan perannya, dan nama lama
-- membuat baris kode yang sudah benar tak perlu ikut disunting.
--
-- ── 2) KOLOM PAMONG ──────────────────────────────────────────────────────────
-- Migrasi 84 sudah mengosongkan isinya dan mengubah pamong menjadi guru. Yang
-- tersisa adalah kolom nol yang tetap dibaca puluhan query, plus satu trigger
-- yang menjaga `require_pamong` tetap sepadan dengan `verify_mode` — dua kolom
-- yang sama-sama tak menentukan apa pun lagi.
--
-- `verify_mode` ikut dibuang: sejak migrasi 84 nilainya selalu 'guru'.
--
-- ── 3) KENAPA `permit_request_classes` BOLEH DIBUANG SEKARANG ────────────────
-- Tabel itu (migrasi 64) menyimpan DAFTAR KELAS yang tercakup sebuah izin. Ia
-- dibutuhkan selama cakupan tak bisa dihitung ulang dengan tepat — dan itu
-- benar selama izin masih berupa "tanggal + jam per hari": menghitung ulang
-- bisa memberi jawaban berbeda dari yang disetujui wali.
--
-- Dengan izin sebagai RENTANG WAKTU, cakupannya tak lagi perlu ditebak: sebuah
-- sesi terlewat bila (tanggal sesi + jam jadwalnya) beririsan dengan rentang
-- izin, dan santrinya peserta kelas itu. Jawabannya sama setiap kali dihitung,
-- oleh siapa pun, tanpa tabel perantara yang bisa basi ketika jadwal berubah.
--
-- ⚠️ SATU-SATUNYA HAL YANG HILANG: cakupan HISTORIS izin lama — daftar kelas
-- apa saja yang dulu tercatat saat izin itu disetujui. Sesudah ini, izin lama
-- dihitung ulang dengan jadwal yang berlaku SEKARANG. Untuk izin yang sudah
-- lewat, angkanya hanya dipakai laporan, bukan penilaian.
--
-- ── MEMBUANG KOLOM ITU TAK SESEDERHANA `DROP COLUMN IF EXISTS` ───────────────
-- Postgres menolaknya selama masih ada objek yang menyebut kolom itu, dan
-- `IF EXISTS` tak menolong sama sekali — yang tak ada bukan kolomnya, melainkan
-- izin untuk membuangnya. Empat penghalang ditemukan saat percobaan pertama,
-- dan semuanya kini dibongkar lebih dulu di tempatnya masing-masing:
--
--   • VIEW `v_classes_missing_pamong` (migrasi 48)      → bagian 2
--   • TRIGGER `trg_sync_require_pamong` (migrasi 62)    → bagian 2
--   • CHECK multi-kolom `chk_classes_verify_non_kbm`    → migrasi 84
--     dan `chk_classes_verify_mode` (migrasi 62 & 67)   → bagian 2
--   • CHECK multi-kolom `chk_permit_jam` (migrasi 66)   → bagian 1
--
-- Yang MENGHILANG DIAM-DIAM justru lebih berbahaya daripada yang menolak:
-- index ikut terbuang tanpa keluhan bersama kolomnya, dan `uq_permit_class_range`
-- (pencegah ajuan ganda) lenyap begitu saja. Ia dipasang ulang di bagian 1.
--
-- Idempotent, dan AMAN DIULANG SETELAH GAGAL DI TENGAH — bagian 1 memeriksa
-- bentuk tabelnya lebih dulu, bukan mengandalkan `IF EXISTS` per baris.
-- Jalankan setelah migrasi 1–85, BERSAMA binary yang sudah tak menyebut
-- kolom-kolom ini. TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh`
-- yang membungkusnya.
-- =============================================================================

-- ── 1) IZIN: dua kolom tanggal + dua kolom jam → dua TIMESTAMP ───────────────
--
-- SELURUHNYA di dalam satu penjagaan `start_date masih ada?`, bukan deretan
-- `IF EXISTS` per baris. Alasannya: bagian ini MENGUBAH BENTUK kolom, jadi
-- setengah jalan pun sudah membuat langkah berikutnya kehilangan pijakan —
-- `ADD COLUMN IF NOT EXISTS mulai_ts` akan dengan patuh membuat ulang kolom
-- kosong yang barusan di-RENAME, lalu UPDATE-nya mencari `start_date` yang
-- sudah tak ada dan seluruh migrasi berhenti.
--
-- Ini bukan kehati-hatian teoretis: migrasi ini memang pernah berhenti di
-- tengah (bagian 2 diblokir sebuah view), dan yang dibutuhkan sesudahnya adalah
-- MENJALANKANNYA LAGI — bukan membereskan separuh keadaan dengan tangan.
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM information_schema.columns
                    WHERE table_name = 'permit_requests'
                      AND column_name = 'start_date') THEN
        RAISE NOTICE 'izin sudah berbentuk rentang — bagian 1 dilewati';
        RETURN;
    END IF;

    -- Pagar lama dilepas dulu. `chk_permit_jam` (migrasi 66) menyebut
    -- start_time DAN end_time sekaligus; CHECK satu-kolom ikut terbuang
    -- bersama kolomnya, yang MULTI-kolom tidak — dan itu menggagalkan DROP di
    -- bawah. Isinya pun sudah tak berlaku: ia mensyaratkan "keduanya NULL atau
    -- keduanya terisi", sementara bentuk barunya NOT NULL keduanya.
    ALTER TABLE permit_requests DROP CONSTRAINT IF EXISTS chk_permit_jam;

    ALTER TABLE permit_requests ADD COLUMN mulai_ts   TIMESTAMP;
    ALTER TABLE permit_requests ADD COLUMN selesai_ts TIMESTAMP;

    -- Jam kosong = izin sehari penuh → 00:00 sampai 23:59:59 pada tanggalnya.
    -- `end_date` kosong = izin sehari → memakai `start_date`.
    UPDATE permit_requests
       SET mulai_ts   = start_date + COALESCE(start_time, TIME '00:00'),
           selesai_ts = COALESCE(end_date, start_date)
                        + COALESCE(end_time, TIME '23:59:59');

    -- Jaring pengaman: rentang terbalik tak boleh lolos (data lama yang aneh).
    UPDATE permit_requests SET selesai_ts = mulai_ts WHERE selesai_ts < mulai_ts;

    ALTER TABLE permit_requests ALTER COLUMN mulai_ts   SET NOT NULL;
    ALTER TABLE permit_requests ALTER COLUMN selesai_ts SET NOT NULL;

    ALTER TABLE permit_requests DROP COLUMN start_date;
    ALTER TABLE permit_requests DROP COLUMN end_date;
    ALTER TABLE permit_requests DROP COLUMN start_time;
    ALTER TABLE permit_requests DROP COLUMN end_time;

    ALTER TABLE permit_requests RENAME COLUMN mulai_ts   TO start_time;
    ALTER TABLE permit_requests RENAME COLUMN selesai_ts TO end_time;
END $$;

ALTER TABLE permit_requests DROP CONSTRAINT IF EXISTS chk_permit_rentang;
ALTER TABLE permit_requests ADD CONSTRAINT chk_permit_rentang
    CHECK (end_time >= start_time);

-- Antrean & pencarian izin selalu menyaring rentangnya.
CREATE INDEX IF NOT EXISTS idx_permit_rentang
    ON permit_requests (start_time, end_time);

-- Pencegah ajuan ganda, DIPASANG ULANG. Aslinya `uq_permit_class_range`
-- (migrasi 46) atas (user_id, class_id, start_date) — ia lenyap tanpa suara
-- bersama `start_date`, karena Postgres membuang index yang kolomnya hilang
-- tanpa mengeluh. Yang hilang begitu bukan cuma index, melainkan aturannya:
-- tanpa ini santri bisa mengirim ajuan yang sama berkali-kali dan wali kelas
-- melihat antrean berisi baris kembar.
--
-- Syarat `pamong_status <> 'rejected'` dibuang bersama kolomnya; sisanya sama
-- persis — ajuan yang SUDAH DITOLAK dikecualikan supaya santri boleh mengajukan
-- ulang setelah perbaikan.
DROP INDEX IF EXISTS uq_permit_class_range;
CREATE UNIQUE INDEX IF NOT EXISTS uq_permit_class_range
    ON permit_requests (user_id, class_id, start_time)
    WHERE guru_status <> 'rejected' AND class_id IS NOT NULL;

-- ── 2) KOLOM PAMONG ──────────────────────────────────────────────────────────
--
-- URUTANNYA PENTING. Postgres menolak `DROP COLUMN` selama masih ada objek lain
-- yang menyebut kolom itu ("cannot drop column ... because other objects depend
-- on it"), dan `IF EXISTS` sama sekali tak menolong — yang tak ada bukan
-- kolomnya, melainkan izin untuk membuangnya. Jadi setiap penyebutnya harus
-- dibongkar lebih dulu:
--
--   a. VIEW    — satu-satunya yang benar-benar memblokir (v_classes_missing_pamong).
--   b. TRIGGER & FUNCTION — menyebut kolomnya di badan fungsi.
--   c. CHECK constraint MULTI-KOLOM — CHECK satu-kolom ikut terbuang sendiri
--      bersama kolomnya, tapi yang menyebut DUA kolom (`category` DAN
--      `verify_mode`) tidak, dan itu yang menggagalkan langkah verify_mode.
--
-- INDEX tak perlu diapa-apakan: ia memang ikut terhapus otomatis.

-- a. View pemantau kelas tanpa pamong (migrasi 48) — pertanyaannya sendiri
--    sudah tak punya arti sejak perannya dihapus.
DROP VIEW IF EXISTS v_classes_missing_pamong;

-- b. Trigger penjaga kesepadanan require_pamong ↔ verify_mode (migrasi 62).
DROP TRIGGER IF EXISTS trg_sync_require_pamong ON classes;
DROP FUNCTION IF EXISTS sync_require_pamong();

-- c. CHECK yang menyebut verify_mode BERSAMA kolom lain (migrasi 62 & 67).
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_mode;
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_non_kbm;

-- Penanda pengingat WA tetap dipakai — hanya penerimanya yang berubah, dari
-- pamong kelas ke WALI kelas (lihat repository::sesi_perlu_pengingat). Namanya
-- ikut diganti supaya tak ada kolom hidup yang menyebut peran yang tak ada.
-- Dilakukan SEBELUM penyapuan di bawah, yang membuang apa pun berawalan
-- 'pamong' — termasuk kolom ini bila namanya belum diganti.
-- RENAME tak punya bentuk IF EXISTS, jadi dijaga sendiri agar tetap idempotent.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.columns
                WHERE table_name = 'class_sessions'
                  AND column_name = 'pamong_reminded_at') THEN
        ALTER TABLE class_sessions RENAME COLUMN pamong_reminded_at TO reminded_at;
    END IF;
END $$;

-- Penyapuan, bukan daftar tetap: kolom pamong lahir di sembilan migrasi berbeda
-- (2, 17, 29, 30, 33, 62, …) dan sebagian lewat RENAME, jadi daftar yang
-- ditulis tangan hampir pasti melewatkan satu — `permit_requests.pamong_note`
-- nyaris lolos begitu. Yang dicari bentuknya, bukan namanya satu per satu.
DO $$
DECLARE k RECORD;
BEGIN
    FOR k IN
        SELECT c.table_name, c.column_name
          FROM information_schema.columns c
          JOIN information_schema.tables t
            ON t.table_schema = c.table_schema AND t.table_name = c.table_name
           -- Hanya TABEL. `information_schema.columns` juga memuat kolom VIEW,
           -- dan `ALTER TABLE ... DROP COLUMN` atas sebuah view langsung gagal.
           AND t.table_type = 'BASE TABLE'
         WHERE c.table_schema = 'public'
           AND c.column_name LIKE 'pamong%'
         ORDER BY c.table_name, c.column_name
    LOOP
        EXECUTE format('ALTER TABLE %I DROP COLUMN %I', k.table_name, k.column_name);
        RAISE NOTICE 'dibuang: %.%', k.table_name, k.column_name;
    END LOOP;
END $$;

-- Dua kolom sisa yang namanya tak berawalan 'pamong' tapi maknanya ikut mati:
-- keduanya hanya menjawab "perlu tahap pamong atau tidak".
ALTER TABLE classes DROP COLUMN IF EXISTS require_pamong;
ALTER TABLE classes DROP COLUMN IF EXISTS verify_mode;

-- ── 3) Cakupan izin per kelas ────────────────────────────────────────────────
DROP TABLE IF EXISTS permit_request_classes;

ANALYZE permit_requests;
ANALYZE attendances;
ANALYZE classes;
ANALYZE class_sessions;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   -- Bentuk baru kolom izin (harus "timestamp without time zone", 2 baris):
--   SELECT column_name, data_type FROM information_schema.columns
--    WHERE table_name = 'permit_requests' AND column_name IN ('start_time','end_time');
--
--   -- Tak ada lagi kolom berawalan pamong di mana pun (harus 0 baris):
--   SELECT table_name, column_name FROM information_schema.columns
--    WHERE column_name LIKE 'pamong%' OR column_name = 'require_pamong'
--       OR column_name = 'verify_mode';
--
--   -- Tabel cakupan sudah hilang (harus NULL):
--   SELECT to_regclass('permit_request_classes');
--
--   -- View pemantau pamong sudah hilang (harus NULL):
--   SELECT to_regclass('v_classes_missing_pamong');
--
--   -- Pencegah ajuan ganda TERPASANG KEMBALI (harus 1 baris) — ini yang paling
--   -- mudah hilang tanpa disadari, karena index terbuang tanpa keluhan:
--   SELECT indexname FROM pg_indexes WHERE indexname = 'uq_permit_class_range';
--
--   -- Pagar lama sudah tak ada (harus 0 baris):
--   SELECT conname FROM pg_constraint
--    WHERE conname IN ('chk_permit_jam', 'chk_classes_verify_mode',
--                      'chk_classes_verify_non_kbm');
--
--   -- Contoh izin: rentangnya masuk akal (mulai <= selesai, sehari penuh
--   -- terlihat 00:00 → 23:59:59):
--   SELECT id, user_id, type, start_time, end_time FROM permit_requests
--    ORDER BY id DESC LIMIT 10;
-- =============================================================================
