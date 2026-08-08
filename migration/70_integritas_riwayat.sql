-- =============================================================================
-- 70_integritas_riwayat.sql — Index yang tak pernah terbuat, pengingat WA
-- ganda, dan riwayat yang bisa lenyap bersama satu DELETE user.
--
-- BAGIAN 1 — INDEX MIGRASI 68 YANG DILEWATI DIAM-DIAM
-- `CREATE INDEX IF NOT EXISTS idx_att_user_date ON attendances (user_id,
-- scan_date)` di migrasi 68 TIDAK PERNAH berjalan: nama itu sudah dipakai
-- migrasi 2 untuk index yang isinya berbeda — (user_id, scanned_at DESC).
-- `IF NOT EXISTS` mencocokkan NAMA, bukan definisi, jadi Postgres melewatinya
-- tanpa sepatah pun peringatan. Query verifikasi di migrasi 68 pun ikut
-- tertipu, karena ia memeriksa nama dan nama itu memang ada.
--
-- Yang menyedihkan: migrasi 45 sudah menuliskan peringatan tentang jebakan ini
-- persis, lalu migrasi 68 menabraknya.
--
-- Index m2 TIDAK dibuang di sini. Ia melayani hal yang berbeda — riwayat
-- santri diurut `scanned_at DESC` — dan membuang index dari tabel terpanas
-- tanpa bukti EXPLAIN dari data produksi adalah menukar satu tebakan dengan
-- tebakan lain. Yang dikerjakan migrasi ini hanya membuat index yang memang
-- belum ada, dengan nama yang tidak bertabrakan.
--
-- CATATAN: tabrakan kedua di migrasi 68, `idx_point_logs_user_created`
-- (m45: (user_id, created_at DESC) vs m68: (user_id, created_at)), SENGAJA
-- dibiarkan. Btree bisa dipindai dua arah, jadi kedua definisi itu melayani
-- query yang sama; yang m68 dilewati, tapi yang m45 sudah ada dan cukup.
-- Membuat duplikatnya hanya menambah beban tulis tanpa menambah kemampuan.
--
-- BAGIAN 2 — SATU JALUR PENGINGAT WA, BUKAN DUA
-- `pamong_notified_at` (migrasi 30) dan `pamong_reminded_at` (migrasi 67)
-- melayani maksud yang sama: menandai bahwa pamong sudah diingatkan tentang
-- sesi yang akan mulai. Keduanya dipakai DUA task berkala berbeda yang tak
-- saling tahu, sehingga satu pamong menerima dua WhatsApp untuk satu sesi.
-- Task lama sudah dihapus dari main.rs. KOLOMNYA TIDAK dibuang di sini —
-- lihat Bagian 2 dan migrasi 71 (pola expand/contract).
--
-- BAGIAN 3 — RIWAYAT & KEUANGAN TAK IKUT TERHAPUS BERSAMA USER
-- `attendances`, `point_logs`, `bills`, dan kawan-kawan memakai ON DELETE
-- CASCADE dari `users`. Satu `DELETE FROM users WHERE id = …` — salah klik di
-- halaman kontrol pengguna, atau satu baris SQL saat membersihkan data uji —
-- menghapus seluruh jejak kehadiran, seluruh riwayat poin, dan seluruh tagihan
-- termasuk yang sudah lunas. Tak ada galat, tak ada sisa, tak ada cara
-- mengembalikan.
--
-- Filosofi yang benar sudah ada di kodebase ini: migrasi 51 sengaja memilih
-- `ON DELETE SET NULL` untuk `point_logs.attendance_id` dengan alasan
-- "menghapus absensi tidak boleh menghapus jejak poin". Alasan yang sama
-- berlaku berlipat untuk users — hanya saja tak pernah diterapkan ke sana.
--
-- RESTRICT membuat penghapusan user GAGAL selama masih ada riwayatnya. Itu
-- bukan efek samping, itu maksudnya: user tidak dihapus, cukup
-- `is_active = FALSE` (kolomnya sudah ada dan sudah dipakai).
--
-- Yang TETAP cascade dan sengaja tidak diubah:
--   sessions            — sesi login, bukan riwayat. User hilang, tokennya
--                         memang harus ikut hilang.
--   class_participants  — keanggotaan kelas, keadaan sekarang, bukan jejak.
-- Kolom pelaku (`verified_by`, `given_by`, `pamong_by`, …) sudah SET NULL
-- sejak awal: barisnya bertahan, yang hilang cuma penunjuk ke pelakunya.
--
-- BAGIAN 4 — SEED KELUAR DARI RANTAI MIGRASI
-- Lihat catatan di akhir berkas.
--
-- Idempotent. Jalankan setelah migrasi 1–69.
-- =============================================================================

-- ═ 1) Index rekap per santri per TANGGAL (yang gagal dibuat migrasi 68) ══════
CREATE INDEX IF NOT EXISTS idx_att_user_scan_date
    ON attendances (user_id, scan_date);

-- ═ 2) Penanda pengingat WA versi lama — SENGAJA BELUM DIBUANG ═══════════════
--
-- Versi pertama migrasi ini menjalankan
--     ALTER TABLE class_sessions DROP COLUMN IF EXISTS pamong_notified_at;
-- dan itu KELIRU. Migrasi di proyek ini dijalankan MANUAL, sedangkan kode
-- di-deploy otomatis saat push — jadi tak ada jaminan mana yang lebih dulu.
-- Yang terjadi: kolomnya dibuang selagi biner LAMA masih hidup, task
-- `claim_due_pamong_reminders` tetap berjalan tiap 5 menit dengan
--     UPDATE class_sessions SET pamong_notified_at = NOW()
--      WHERE … AND pamong_notified_at IS NULL …
-- lalu gagal `column "pamong_notified_at" does not exist` — berulang, dan tiap
-- kegagalan ikut mengirim alert Telegram lewat report_background_error.
--
-- Aturannya (expand/contract): SATU rilis hanya boleh MENAMBAH atau berhenti
-- memakai; MEMBUANG menyusul di rilis berikutnya, setelah tak ada lagi biner
-- yang memakainya. Kolom nganggur berisi NULL tak memakan apa pun; log error
-- yang membanjiri Telegram memakan perhatian orang.
--
-- Kolom ini dibuang di `71_drop_pamong_notified_at.sql`. JALANKAN ITU SETELAH
-- biner baru benar-benar berjalan di produksi (cek: log memuat "Pengingat
-- sesi: N pesan terkirim ke pamong", bukan "Pengingat sesi pamong gagal").

-- PEMULIHAN bila versi pertama migrasi ini sudah terlanjur dijalankan dan
-- kolomnya sudah dibuang: baris di bawah mengembalikannya, sehingga biner lama
-- yang masih hidup berhenti menghantam log & Telegram seketika. Idempotent —
-- tak melakukan apa pun bila kolomnya memang masih ada.
ALTER TABLE class_sessions ADD COLUMN IF NOT EXISTS pamong_notified_at TIMESTAMPTZ;

-- ═ 3) CASCADE → RESTRICT untuk riwayat, audit, dan keuangan ═════════════════
--
-- Nama constraint TIDAK ditulis tangan. Sebagiannya lahir dari CREATE TABLE
-- (`attendances_user_id_fkey`), sebagian dari ALTER TABLE belakangan, dan yang
-- dibuat manual bisa bernama apa saja. Menebak namanya berarti migrasi ini
-- diam-diam tak melakukan apa-apa pada database yang namanya berbeda — persis
-- kegagalan diam yang sedang diperbaiki di Bagian 1.
--
-- Jadi constraint-nya DICARI dari katalog: FK pada (tabel, kolom) yang menunjuk
-- ke `users`, lalu dibangun ulang dengan aksi yang diinginkan. Sudah RESTRICT?
-- dilewati, jadi menjalankan ulang migrasi ini tak melakukan apa pun.
DO $$
DECLARE
    sasaran CONSTANT text[][] := ARRAY[
        -- tabel,            kolom,          aksi
        ['attendances',      'user_id',      'RESTRICT'],
        ['point_logs',       'user_id',      'RESTRICT'],
        ['permit_requests',  'user_id',      'RESTRICT'],
        ['gate_logs',        'user_id',      'RESTRICT'],
        ['hafalan_logs',     'user_id',      'RESTRICT'],
        ['ipk_history',      'user_id',      'RESTRICT'],
        ['weekly_rewards',   'user_id',      'RESTRICT'],
        ['bills',            'user_id',      'RESTRICT'],
        -- Pengaju izin: barisnya riwayat, tapi kolomnya nullable dan yang
        -- ditunjuk cuma PELAKU — cukup dilepas, tak perlu menghalangi.
        ['permit_requests',  'requested_by', 'SET NULL']
    ];
    i          int;
    tabel      text;
    kolom      text;
    aksi       text;
    nama_con   text;
    aksi_kini  char;
    aksi_mau   char;
BEGIN
    FOR i IN 1 .. array_length(sasaran, 1) LOOP
        tabel := sasaran[i][1];
        kolom := sasaran[i][2];
        aksi  := sasaran[i][3];
        aksi_mau := CASE aksi WHEN 'RESTRICT' THEN 'r' ELSE 'n' END;

        -- Tabel belum ada (urutan migrasi tak lazim) → lewati, jangan gagal.
        IF to_regclass(tabel) IS NULL THEN
            RAISE NOTICE 'lewati %.% — tabelnya tak ada', tabel, kolom;
            CONTINUE;
        END IF;

        SELECT c.conname, c.confdeltype
          INTO nama_con, aksi_kini
          FROM pg_constraint c
          JOIN pg_attribute a
            ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
         WHERE c.contype = 'f'
           AND c.conrelid = to_regclass(tabel)
           AND c.confrelid = to_regclass('users')
           AND array_length(c.conkey, 1) = 1
           AND a.attname = kolom
         LIMIT 1;

        IF nama_con IS NULL THEN
            RAISE NOTICE 'lewati %.% — tak ada FK ke users', tabel, kolom;
            CONTINUE;
        END IF;
        IF aksi_kini = aksi_mau THEN
            CONTINUE;  -- sudah benar; migrasi ini idempotent
        END IF;

        EXECUTE format('ALTER TABLE %I DROP CONSTRAINT %I', tabel, nama_con);
        EXECUTE format(
            'ALTER TABLE %I ADD CONSTRAINT %I FOREIGN KEY (%I) '
            'REFERENCES users(id) ON DELETE %s',
            tabel, nama_con, kolom, aksi
        );
        RAISE NOTICE 'FK %.% → ON DELETE %', tabel, kolom, aksi;
    END LOOP;
END $$;

ANALYZE attendances;

-- =============================================================================
-- BAGIAN 4 — CATATAN TENTANG SEED (tak ada DDL di sini)
--
-- `3_seed_users.sql` DIPINDAH keluar dari folder ini menjadi `dev/seed_users.sql`
-- dan tidak lagi bagian dari rantai migrasi. Tiga alasan:
--
--   1. Ia memuat hash bcrypt dari password yang tertulis lengkap di komentar
--      berkas yang sama, dan ikut dijalankan di produksi.
--   2. Ia menyisipkan role 'teacher', yang DILARANG CHECK sejak migrasi 38 —
--      artinya rantai migrasi pada database BARU pasti berhenti di sana.
--   3. Ia tak dibutuhkan: `service::auth::ensure_seed_admin` sudah membuat
--      admin pertama saat tabel users kosong, dengan sandi dari env
--      ADMIN_PASSWORD dan penolakan tegas bila LEPTOS_ENV=PROD tanpa env itu.
--
-- Database yang SUDAH menjalankan migrasi 3 tak perlu tindakan apa pun —
-- catatannya di `schema_migrations` tetap ada, berkasnya saja yang pindah.
-- Bila akun contoh itu masih hidup di produksi, nonaktifkan:
--   UPDATE users SET is_active = FALSE
--    WHERE username IN ('admin','ustadz.ahmad','pamong.budi','santri.ali','ortu.ali')
--      AND password_hash = '$2b$10$XnKUkKbMB7Ubn/D7zlmiIOlp82zDFhOc/Odi3ebcgWTXC3IIYlVzu';
--
-- Verifikasi migrasi ini — periksa DEFINISI, bukan nama (pelajaran Bagian 1):
--   SELECT indexname, indexdef FROM pg_indexes
--    WHERE tablename = 'attendances' AND indexdef LIKE '%scan_date%';
--
--   SELECT c.conrelid::regclass AS tabel, a.attname AS kolom, c.confdeltype
--     FROM pg_constraint c
--     JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
--    WHERE c.contype = 'f' AND c.confrelid = 'users'::regclass
--    ORDER BY 1, 2;   -- 'r' = RESTRICT, 'n' = SET NULL, 'c' = CASCADE
--
--   SELECT column_name FROM information_schema.columns
--    WHERE table_name = 'class_sessions' AND column_name LIKE 'pamong_%';
-- =============================================================================
