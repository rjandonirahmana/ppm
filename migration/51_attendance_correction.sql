-- =============================================================================
-- 51_attendance_correction.sql — Jalur KOREKSI absensi.
--
-- MASALAH:
--   Perangkat mati, listrik padam, atau santri lupa kartu → ia tak bisa tap →
--   `run_auto_absent` menandainya ALPA dan memotong poinnya. Staf yang tahu itu
--   keliru lalu menandainya hadir, tapi `mark_manual_present` memakai
--   `ON CONFLICT (user_id, class_session_id) DO NOTHING` — baris alpa sudah
--   ada, jadi INSERT tak melakukan apa-apa. Staf melihat pesan sukses padahal
--   TIDAK ADA yang berubah. Dan tak ada satu pun UPDATE/DELETE absensi di
--   seluruh kode.
--
--   Akibatnya alpa keliru itu permanen, poinnya hilang permanen, tanpa banding.
--   Satu hari perangkat mati = sekelas kena, tak seorang pun bisa membatalkan.
--
-- YANG DITAMBAHKAN DI SINI:
--   `point_logs.attendance_id` — tautan dari catatan poin ke absensi asalnya.
--   Tanpa tautan ini, mengoreksi status tak bisa menarik balik poinnya: kita
--   tak tahu baris log mana milik absensi mana (mencocokkan lewat `reason`
--   berupa teks jelas rapuh).
--
--   Dengan tautan ini, koreksi cukup MENGHAPUS log poin lama → trigger
--   `trg_point_logs_balance` (migrasi 32) otomatis mengembalikan saldonya.
--   Tak perlu aritmetika manual yang bisa salah.
--
-- SIAPA YANG BOLEH MENGOREKSI: hanya GURU PENGISI atau PAMONG yang bertugas di
--   sesi itu (ditegakkan di query, lihat repository::correct_attendance).
--   Bukan admin, bukan wali kelas lain — yang tahu apa yang sebenarnya terjadi
--   di ruangan hanyalah yang bertugas saat itu.
--
-- Idempotent. Jalankan setelah migrasi 1–50.
-- =============================================================================

-- ═ 1) Tautan log poin → absensi ══════════════════════════════════════════════
-- ON DELETE SET NULL, bukan CASCADE: menghapus absensi TIDAK boleh diam-diam
-- menghapus jejak poinnya. Log tetap ada (saldo tak berubah), hanya tautannya
-- yang lepas.
ALTER TABLE point_logs
    ADD COLUMN IF NOT EXISTS attendance_id BIGINT REFERENCES attendances(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_point_logs_attendance
    ON point_logs (attendance_id) WHERE attendance_id IS NOT NULL;

-- ═ 2) Data lama tak bisa ditautkan surut ════════════════════════════════════
-- Log poin yang sudah ada tak menyimpan asal absensinya, dan mencocokkan lewat
-- (user, waktu, teks alasan) bisa salah tebak — lebih buruk daripada tak
-- menautkan. Konsekuensinya: absensi LAMA yang dikoreksi tak akan mengembalikan
-- poin secara otomatis; poinnya perlu disesuaikan manual lewat menu poin.
--
-- Untuk melihat berapa banyak yang terdampak:
--   SELECT COUNT(*) FROM point_logs
--    WHERE category IN ('attendance','discipline') AND attendance_id IS NULL;

-- ═ 3) Kolom penanda koreksi ══════════════════════════════════════════════════
-- Supaya jejaknya jujur: baris yang pernah diubah manusia harus bisa dibedakan
-- dari yang murni hasil scan/job.
ALTER TABLE attendances
    ADD COLUMN IF NOT EXISTS corrected_by BIGINT REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE attendances
    ADD COLUMN IF NOT EXISTS corrected_at TIMESTAMPTZ;

COMMENT ON COLUMN attendances.corrected_by IS
    'Guru/pamong bertugas yang mengoreksi status absensi ini. NULL = belum pernah dikoreksi.';
