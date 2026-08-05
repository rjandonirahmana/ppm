-- =============================================================================
-- 56_fix_scan_date.sql — Selaraskan attendances.scan_date dengan session_date.
--
-- MASALAH
-- Sebagian baris absensi lama punya `scan_date` yang diambil dari KAPAN staf
-- menandai (`scanned_at`), bukan dari TANGGAL SESI yang ditandai. Akibatnya
-- satu santri bisa punya beberapa baris untuk jadwal yang sama dengan
-- `scan_date` yang sama padahal sesinya berbeda hari — mis. tiga alfa untuk
-- sesi 26, 27, dan 28 Juli semuanya tercap 29 Juli (hari staf mengklik).
--
-- Itulah yang membuat migrasi 42 tak pernah selesai: `CREATE UNIQUE INDEX
-- uq_attendance_schedule_daily (user_id, class_schedule_id, scan_date)` gagal
-- karena baris-baris itu bertabrakan, sementara langkah DELETE di migrasi 42
-- tidak menyentuhnya (ia men-dedup dengan kunci yang JUGA memuat
-- class_session_id, sehingga baris yang beda sesi dianggap bukan duplikat).
--
-- KENAPA session_date YANG BENAR
--   • `class_sessions.session_date` tidak pernah di-UPDATE di kode mana pun,
--     jadi ia catatan yang stabil;
--   • logika auto-absent aplikasi sendiri sudah membandingkan
--     `a2.scan_date = s.session_date` (repository/attendance.rs);
--   • seluruh jalur insert yang berlaku sekarang — mark_manual_present,
--     mark_attendance_bulk, run_auto_absent, dan pemberian izin — semuanya
--     sudah mengisi `scan_date` dari `s.session_date`.
--
-- Jadi ini murni membereskan sisa data lama; tak ada bug yang masih berjalan.
--
-- URUTAN JALAN
-- Untuk database yang migrasi 42-nya BELUM selesai (produksi saat ini),
-- jalankan berkas ini LEBIH DULU, baru 42 — tanpa itu 42 tetap gagal di
-- langkah index. Pada database baru urutan normal (42 lalu 56) tetap aman:
-- saat 42 berjalan, tabel attendances masih kosong.
--
-- Idempotent: dijalankan berulang, kali kedua memperbarui 0 baris.
-- =============================================================================

UPDATE attendances a
   SET scan_date = s.session_date
  FROM class_sessions s
 WHERE s.id = a.class_session_id
   AND a.scan_date IS DISTINCT FROM s.session_date;

-- Verifikasi (harus 0):
--   SELECT count(*) FROM attendances a JOIN class_sessions s
--     ON s.id = a.class_session_id
--    WHERE a.scan_date IS DISTINCT FROM s.session_date;
