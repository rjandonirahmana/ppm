-- =============================================================================
-- 67_bersihkan_dan_verifikasi.sql — Buang tabel mati; non-KBM cukup satu tahap.
--
-- BAGIAN 1 — TABEL YANG DIBUANG
-- Ketiganya tak disentuh satu baris kode pun (dicek dengan grep ke seluruh
-- src/), tak jadi tujuan foreign key mana pun, dan isinya sisa percobaan lama:
--
--   sessions         0 baris. Tabel sesi LOGIN dari sebelum autentikasi pindah
--                    ke JWT. Sejak itu tak pernah ditulis maupun dibaca.
--                    Namanya berbahaya: mirip `class_sessions` yang justru inti
--                    aplikasi, jadi mudah tertukar saat membaca skema.
--   academic_terms   1 baris. Digantikan `academic_semesters`.
--   complaints       0 baris. Fitur pengaduan yang tak pernah jadi.
--
-- Tabel kosong lain SENGAJA DIPERTAHANKAN karena kodenya hidup dan tinggal
-- menunggu dipakai: gate_logs (ditulis repository::gate), guest_visits (buku
-- tamu), ipk_history, weekly_rewards, hafalan_logs.
--
-- BAGIAN 2 — VERIFIKASI KELAS NON-KBM
-- Kelas non-KBM dan Bacaan tak memerlukan verifikasi dua langkah: cukup pamong
-- yang bertugas di sesi itu. Dua langkah hanya masuk akal di KBM, tempat ada
-- guru pengajar DAN pamong pengawas. Tanpa aturan ini, absensi piket/apel bisa
-- menggantung menunggu tahap kedua yang tak pernah ada petugasnya.
--
-- `require_pamong` ikut menyesuaikan sendiri lewat trigger migrasi 62.
--
-- Idempotent. Jalankan setelah migrasi 1–66.
-- =============================================================================

DROP TABLE IF EXISTS sessions;
DROP TABLE IF EXISTS academic_terms;
DROP TABLE IF EXISTS complaints;

UPDATE classes SET verify_mode = 'pamong'
 WHERE category <> 'kbm' AND verify_mode <> 'pamong';

ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_non_kbm;
ALTER TABLE classes ADD CONSTRAINT chk_classes_verify_non_kbm
    CHECK (category = 'kbm' OR verify_mode = 'pamong');

-- BAGIAN 3 — PENGINGAT PENUGASAN SESI
-- Pamong kelas diingatkan lewat WhatsApp satu jam sebelum sesi KBM dimulai,
-- supaya sempat menunjuk guru pengajar dan pamong bertugas. Kolom ini yang
-- menjaga pengingatnya terkirim SEKALI: tugas latar berjalan tiap beberapa
-- menit, jadi tanpa penanda, satu sesi akan mengirim belasan pesan.
--
-- Disimpan di tabel, bukan di Redis: pengingat yang sudah terkirim adalah
-- fakta tentang sesi itu, dan harus tetap benar walau prosesnya di-restart.
ALTER TABLE class_sessions ADD COLUMN IF NOT EXISTS pamong_reminded_at TIMESTAMPTZ;

-- Verifikasi:
--   SELECT id, name, category, verify_mode, require_pamong FROM classes ORDER BY id;
--   SELECT tablename FROM pg_tables WHERE schemaname='public' ORDER BY 1;
--   SELECT id, title, session_date, pamong_reminded_at FROM class_sessions
--    WHERE pamong_reminded_at IS NOT NULL ORDER BY pamong_reminded_at DESC LIMIT 10;
