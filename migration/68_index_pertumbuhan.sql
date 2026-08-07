-- =============================================================================
-- 68_index_pertumbuhan.sql — Index untuk tabel yang AKAN membesar.
--
-- KENAPA SEKARANG, PADAHAL DATANYA MASIH KECIL
-- Hari ini `attendances` berisi 47 baris dan seluruh query di atasnya dilayani
-- seq-scan dalam waktu yang tak terukur. Tapi bentuk pertumbuhannya sudah
-- diketahui: 3.000 santri × beberapa sesi per hari ≈ sejuta baris per tahun
-- ajaran. Index yang dibuat saat tabel masih kecil selesai seketika; index yang
-- sama pada tabel sejuta baris mengunci tulis cukup lama untuk terasa saat
-- absensi berlangsung.
--
-- YANG TIDAK DILAKUKAN, DAN KENAPA
--   • Partisi tabel — pemecahan per tanggal baru masuk akal di puluhan juta
--     baris; sekarang ia hanya menambah rumit tanpa satu pun query yang lebih
--     cepat.
--   • Materialized view / denormalisasi — menyimpan hasil join berarti punya
--     dua sumber kebenaran yang harus dijaga sinkron. Rekap kami dibaca
--     beberapa kali sehari, bukan ribuan kali per detik; tak sepadan.
--   • Index untuk SETIAP kolom foreign key — ada 18 yang belum terindeks, tapi
--     kebanyakan (verified_by, corrected_by, uploaded_by, …) tak pernah jadi
--     syarat pencarian; hanya dibaca lewat id barisnya sendiri. Index di sana
--     memperlambat tulis tanpa mempercepat baca apa pun.
--
-- Yang dipilih di bawah semuanya punya query nyata sebagai alasan.
--
-- Idempotent. Jalankan setelah migrasi 1–67.
-- =============================================================================

-- attendances — tabel yang tumbuh paling cepat.
--
-- 1) Dedup tap RFID & rekap per jadwal: `class_schedule_id` + `scan_date`
--    dipakai bersama di `attendance_exists_today` dan run_auto_absent
--    (NOT EXISTS ... a2.class_schedule_id = ... AND a2.scan_date = ...).
CREATE INDEX IF NOT EXISTS idx_att_schedule_date
    ON attendances (class_schedule_id, scan_date);

-- 2) Rekap mingguan & rapor santri: seluruhnya menyaring per santri lalu
--    rentang tanggal.
CREATE INDEX IF NOT EXISTS idx_att_user_date
    ON attendances (user_id, scan_date);

-- 3) Antrean verifikasi: baris yang MASIH menunggu. Index PARSIAL — begitu
--    absensi selesai diverifikasi ia keluar dari index, jadi ukurannya tetap
--    sebesar antrean (puluhan) alih-alih sebesar tabel (jutaan).
CREATE INDEX IF NOT EXISTS idx_att_pending_verify
    ON attendances (class_session_id)
    WHERE verify_status = 'pending';

-- point_logs — tumbuh seiring absensi (satu baris per poin).
-- Papan poin & rekap menjumlahkan per santri dalam rentang tanggal.
CREATE INDEX IF NOT EXISTS idx_point_logs_user_created
    ON point_logs (user_id, created_at);

-- class_sessions — dibaca jalur RFID tiap tap (JOIN ke jadwal + tanggal hari
-- ini) dan job auto-absent. uq_session_schedule_date (migrasi 52) sudah
-- menutupi (class_schedule_id, session_date); yang belum: pencarian per KELAS
-- pada rentang tanggal — dipakai materialisasi izin & daftar sesi kelas.
CREATE INDEX IF NOT EXISTS idx_sessions_class_date
    ON class_sessions (class_id, session_date);

-- class_session_chats — chat sesi selalu dibaca "semua pesan sesi ini".
-- Satu-satunya FK tak terindeks yang benar-benar jadi syarat pencarian.
CREATE INDEX IF NOT EXISTS idx_chat_session
    ON class_session_chats (session_id, id);

-- permit_requests — antrean wali kelas menyaring status + tujuan.
CREATE INDEX IF NOT EXISTS idx_permit_pending_wali
    ON permit_requests (wali_kelas_id)
    WHERE guru_status = 'pending';

-- Statistik dibuat segar setelah index dibuat: planner baru akan memakainya
-- hanya bila ia tahu sebaran datanya. Di produksi 21 dari 27 tabel belum
-- pernah dianalisis sama sekali (autovacuum belum tersentuh karena tabelnya
-- masih kecil), jadi planner selama ini menebak.
ANALYZE;

-- Verifikasi:
--   SELECT indexrelname, idx_scan FROM pg_stat_user_indexes
--    WHERE relname IN ('attendances','point_logs','class_sessions')
--    ORDER BY idx_scan DESC;
