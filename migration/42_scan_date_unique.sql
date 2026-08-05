-- =============================================================================
-- 42_scan_date_unique.sql — Fix attendance duplikat (critical security)
--
-- Tambahkan kolom scan_date (tanggal WIB) untuk UNIQUE constraint yang benar.
-- Masalah: race condition antara attendance_exists_today & insert_attendance.
-- Solusi: UNIQUE(user_id, COALESCE(class_session_id,-1), scan_date) + idempoten.
--
-- Idempotent. Jalankan setelah migrasi 1–41.
-- =============================================================================

-- 1) Tambahkan kolom scan_date (optional awalnya, untuk backfill safe).
ALTER TABLE attendances
    ADD COLUMN IF NOT EXISTS scan_date DATE;

-- 2) Backfill scan_date dari scanned_at (WIB timezone).
-- Timezone Jakarta (UTC+7): SET timezone = 'Asia/Jakarta'
UPDATE attendances
SET scan_date = (scanned_at AT TIME ZONE 'Asia/Jakarta')::date
WHERE scan_date IS NULL;

-- ═ 3) Bereskan duplikat LAMA sebelum memasang UNIQUE ════════════════════════
-- Justru duplikat itulah alasan migrasi ini ada. Tanpa dibereskan dulu,
-- CREATE UNIQUE INDEX gagal dan seluruh rantai migrasi berhenti.
-- Sisakan baris TERTUA (id terkecil) — itu tap pertama yang sebenarnya.
--
-- PERBAIKAN: kunci dedup di bawah HARUS sama persis dengan kunci index yang
-- dipasang di langkah 4, dan dipecah dua mengikuti keduanya. Versi sebelumnya
-- memakai SATU kunci yang juga memuat `class_session_id` — lebih sempit
-- daripada index (a) yang tak memuat kolom itu — sehingga baris yang berbeda
-- hanya di `class_session_id` lolos dari DELETE tapi tetap melanggar index,
-- dan langkah 4 gagal. Itu yang terjadi di produksi: DELETE menyapu 0 baris,
-- lalu CREATE UNIQUE INDEX berhenti dengan galat.
--
-- Catatan: penyebab bentrok di produksi bukan duplikat sungguhan melainkan
-- `scan_date` lama yang tak sinkron dengan `session_date` — itu dibereskan
-- migrasi 56, yang untuk database seperti itu harus dijalankan LEBIH DULU.
-- Dedup di bawah tetap ada sebagai jaring pengaman.

-- (a) Sepadan dengan uq_attendance_schedule_daily.
DELETE FROM attendances a USING attendances b
 WHERE a.id > b.id
   AND a.class_schedule_id IS NOT NULL
   AND b.class_schedule_id IS NOT NULL
   AND a.user_id = b.user_id
   AND a.scan_date = b.scan_date
   AND a.class_schedule_id = b.class_schedule_id;

-- (b) Sepadan dengan uq_attendance_freescan_daily.
DELETE FROM attendances a USING attendances b
 WHERE a.id > b.id
   AND a.class_schedule_id IS NULL AND b.class_schedule_id IS NULL
   AND a.class_session_id IS NULL AND b.class_session_id IS NULL
   AND a.user_id = b.user_id
   AND a.scan_date = b.scan_date;

-- ═ 4) UNIQUE yang BENAR-BENAR mencerminkan aturan dedup aplikasi ════════════
-- Versi awal migrasi ini memakai (user_id, COALESCE(class_session_id,-1),
-- scan_date) — SALAH KOLOM. `attendance_exists_today` mendedup per JADWAL
-- (class_schedule_id), bukan per sesi. Akibatnya:
--   • dua jadwal berbeda yang belum punya sesi di hari sama sama-sama jatuh ke
--     kunci (user, -1, tanggal) → tap kedua ditolak DB padahal sah;
--   • sebaliknya, tap ganda pada SATU jadwal lolos bila sesi keburu dibuat di
--     antara dua tap (kunci berubah dari -1 ke id sesi).
-- Aplikasi dan constraint harus sepakat, jadi indexnya dipecah dua:

-- (a) Tap pada jadwal: satu absensi per (santri, jadwal, hari).
--     Persis yang dicek attendance_exists_today untuk schedule_id = Some(..).
CREATE UNIQUE INDEX IF NOT EXISTS uq_attendance_schedule_daily
    ON attendances (user_id, class_schedule_id, scan_date)
    WHERE class_schedule_id IS NOT NULL;

-- (b) Tap di LUAR jadwal (outside_schedule): satu per (santri, hari).
--     Dibatasi ke baris tanpa sesi, supaya penandaan manual pada sesi ad-hoc
--     (punya class_session_id) tak ikut terkunci — santri boleh hadir di lebih
--     dari satu sesi ad-hoc dalam sehari, dan itu sudah dijaga
--     UNIQUE (user_id, class_session_id) dari migrasi 2.
CREATE UNIQUE INDEX IF NOT EXISTS uq_attendance_freescan_daily
    ON attendances (user_id, scan_date)
    WHERE class_schedule_id IS NULL AND class_session_id IS NULL;

-- Buang index versi lama bila sempat terpasang dari revisi sebelumnya.
DROP INDEX IF EXISTS uq_attendance_daily;

-- ═ 5) scan_date WAJIB terisi ════════════════════════════════════════════════
-- Nullable = celah: insert yang lupa mengisinya lolos, dan UNIQUE menganggap
-- NULL selalu berbeda → duplikat kembali mungkin tanpa ada yang sadar.
UPDATE attendances SET scan_date = (scanned_at AT TIME ZONE 'Asia/Jakarta')::date
 WHERE scan_date IS NULL;
ALTER TABLE attendances
    ALTER COLUMN scan_date SET DEFAULT (NOW() AT TIME ZONE 'Asia/Jakarta')::date;
ALTER TABLE attendances ALTER COLUMN scan_date SET NOT NULL;

-- ═ 6) Verifikasi (jalankan MANUAL di staging) ═══════════════════════════════
-- Berapa baris yang terhapus sebagai duplikat? Bandingkan sebelum/sesudah:
--   SELECT COUNT(*) FROM attendances;
