-- =============================================================================
-- 85_hapus_status_outside_schedule.sql — Status 'outside_schedule' DIBUANG.
--
-- ── APA ITU 'outside_schedule' ───────────────────────────────────────────────
-- Ia lahir di migrasi 7 untuk satu keadaan: santri menempel kartu di perangkat
-- kelas PADA SAAT tak ada jadwalnya yang sedang berlangsung. Sebelum itu tap
-- semacam ini dipaksa jadi 'present' — hadir pada kelas yang tak ada. Statusnya
-- lalu dipakai sebagai "log gerbang": barisnya disimpan tanpa jadwal
-- (class_schedule_id NULL) dengan alasan jejak lalu-lalang tetap berguna.
--
-- ── KENAPA DIBUANG (keputusan pengurus, Ags 2026) ────────────────────────────
-- Jejak lalu-lalang SUDAH punya tempatnya sendiri: gerbang utama mencatatnya di
-- jalurnya sendiri (`toggle_gate`, migrasi 12/49), lengkap dengan arah keluar
-- atau masuk. Yang tersisa di sini hanyalah baris yang menumpang di tabel
-- KEHADIRAN KELAS, dan di sana ia merugikan:
--
--   1. Rekap mingguan menghitungnya bersama 'late' → santri yang cuma lewat di
--      luar jam tampak "terlambat" pada laporan pekanan.
--   2. Ia muncul di riwayat santri sebagai baris kehadiran, padahal tak ada
--      kelas yang dihadiri.
--   3. `run_auto_absent` melewati santri yang "sudah ada catatan" hari itu —
--      jadi SATU tap iseng di luar jam bisa menutupi alfa yang sesungguhnya.
--
-- Aturan barunya sederhana dan sudah berlaku di kode (`service::attendance::
-- record_scan`): tap di luar jam kelas TIDAK dicatat, dan tap di perangkat yang
-- bukan ruang kelasnya (untuk jadwal yang terikat ruang) juga tidak — keduanya
-- dijawab `ok:false` supaya mesin memberi tahu santrinya.
--
-- ── YANG DILAKUKAN ───────────────────────────────────────────────────────────
--   1. HAPUS baris `attendances` berstatus 'outside_schedule' (jumlah & rentang
--      tanggalnya di-RAISE NOTICE — jangan jalankan sambil memalingkan muka).
--   2. Persempit CHECK `attendances_status_check` → lima status yang tersisa.
--
-- ⚠️ BACA DULU SEBELUM MENJALANKAN. Baris yang dihapus tak bisa dikembalikan.
-- Jalankan ini lebih dulu untuk melihat apa yang akan hilang:
--
--   SELECT count(*) AS baris,
--          min(scan_date) AS sejak, max(scan_date) AS sampai,
--          count(DISTINCT user_id) AS santri
--     FROM attendances WHERE status = 'outside_schedule';
--
--   -- Contoh barisnya (periksa: semestinya class_schedule_id NULL semua):
--   SELECT id, user_id, scan_date, class_schedule_id, class_session_id, gate_label
--     FROM attendances WHERE status = 'outside_schedule'
--    ORDER BY scanned_at DESC LIMIT 20;
--
-- Kalau ternyata ada yang class_schedule_id-nya TIDAK NULL, berhenti dan
-- laporkan — itu berarti ada jalur lain yang menulis status ini, dan
-- menghapusnya akan membuang kehadiran kelas yang sungguhan.
--
-- POIN TIDAK TERPENGARUH: 'outside_schedule' berdelta 0 di `DELTA_SQL`, jadi ia
-- tak pernah melahirkan baris `point_logs` — menghapusnya tak menggeser saldo
-- siapa pun. (Kalaupun ada log yatim dari versi lama, `point_logs.attendance_id`
-- memakai ON DELETE SET NULL sehingga saldonya tetap utuh.)
--
-- Idempotent. Jalankan setelah migrasi 1–84.
-- TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh` yang membungkusnya.
-- =============================================================================

DO $$
DECLARE
    n      bigint;
    sejak  date;
    sampai date;
    nyasar bigint;
BEGIN
    SELECT count(*), min(scan_date), max(scan_date)
      INTO n, sejak, sampai
      FROM attendances WHERE status = 'outside_schedule';

    IF n = 0 THEN
        RAISE NOTICE 'Tak ada baris outside_schedule — tak ada yang dihapus.';
    ELSE
        -- Pagar terakhir: baris berstatus ini SEHARUSNYA tak pernah tertaut
        -- jadwal. Kalau ada yang tertaut, asumsi migrasi ini salah dan yang
        -- terhapus bisa jadi kehadiran sungguhan — batalkan seluruhnya.
        SELECT count(*) INTO nyasar
          FROM attendances
         WHERE status = 'outside_schedule' AND class_schedule_id IS NOT NULL;
        IF nyasar > 0 THEN
            RAISE EXCEPTION
                '% baris outside_schedule TERTAUT jadwal — dibatalkan. Periksa dulu, lihat kepala berkas ini.',
                nyasar;
        END IF;

        RAISE NOTICE 'Menghapus % baris outside_schedule (% s/d %).', n, sejak, sampai;
        DELETE FROM attendances WHERE status = 'outside_schedule';
    END IF;
END $$;

ALTER TABLE attendances DROP CONSTRAINT IF EXISTS attendances_status_check;
ALTER TABLE attendances ADD CONSTRAINT attendances_status_check
    CHECK (status IN ('present','late','absent','permit','sick'));

-- Index `uq_attendance_freescan_daily` (migrasi 42) SENGAJA DIBIARKAN. Ia
-- menjaga baris tanpa jadwal DAN tanpa sesi — bentuk yang setelah ini tak ada
-- lagi jalur kodenya, jadi ia kini sekadar jaring pengaman bila suatu saat ada
-- yang menulis langsung ke tabel. Membuangnya tak menghemat apa pun (index
-- parsial atas nol baris) dan hanya melepas satu pagar.

ANALYZE attendances;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   SELECT count(*) FROM attendances WHERE status = 'outside_schedule';  -- 0
--
--   SELECT pg_get_constraintdef(oid) FROM pg_constraint
--    WHERE conrelid = 'attendances'::regclass AND conname = 'attendances_status_check';
--   -- tak boleh lagi memuat 'outside_schedule'.
--
--   -- Saldo poin harus tetap sama dengan jumlah lognya (invarian migrasi 32/72):
--   SELECT u.id FROM users u LEFT JOIN point_logs pl ON pl.user_id = u.id
--    GROUP BY u.id, u.points HAVING u.points <> COALESCE(SUM(pl.delta), 0);
--   -- harus 0 baris.
-- =============================================================================
