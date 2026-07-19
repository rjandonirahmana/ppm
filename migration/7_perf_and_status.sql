-- =============================================================================
-- 7_perf_and_status.sql — Perbaikan AMAN dari audit (additive, tak merusak data
-- lama & tak menuntut perubahan kode lebih dulu):
--   1) Perluas status attendances → tambah 'outside_schedule' (scan di luar jadwal
--      tak lagi dipaksa 'present'). CHECK diperluas dulu; kode di-flip menyusul.
--   2) Index yang belum ada: (user_id,status) & jadwal aktif.
--
-- CATATAN unik-per-hari (celah UNIQUE(user_id,class_session_id) saat session NULL):
--   TIDAK bisa pakai UNIQUE INDEX fungsional atas DATE(scanned_at) / (… AT TIME ZONE)
--   — ekspresi itu STABLE, bukan IMMUTABLE, jadi Postgres MENOLAKnya untuk index.
--   Solusi benar = kolom tersimpan `scan_date DATE` (diisi app = tanggal WIB) lalu
--   UNIQUE(user_id, COALESCE(class_session_id,-1), scan_date). Itu butuh kolom +
--   perubahan insert + backfill → dibuat terpisah setelah disepakati (lihat chat).
--
-- Idempotent. Jalankan setelah migrasi 1–6.
-- =============================================================================

-- 1) Perluas status kehadiran (additive; nilai lama tetap valid).
ALTER TABLE attendances DROP CONSTRAINT IF EXISTS attendances_status_check;
ALTER TABLE attendances ADD CONSTRAINT attendances_status_check
    CHECK (status IN ('present','late','outside_schedule','absent','permit','sick'));

-- 2) Index yang belum ada.
-- Papan poin / filter per-status per-santri (mis. hitung hadir/izin/alfa).
CREATE INDEX IF NOT EXISTS idx_att_user_status ON attendances (user_id, status);

-- Lookup jadwal aktif pada rentang tanggal (record_scan → active_schedule_now).
CREATE INDEX IF NOT EXISTS idx_schedule_active
    ON class_schedules (class_id, status, start_date, end_date);

-- Catatan: idx_att_user_date (user_id, scanned_at DESC) SUDAH ada di migrasi 2,
-- jadi kebutuhan "index scanned_at" sebagian besar sudah tercakup.
