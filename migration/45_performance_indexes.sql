-- =============================================================================
-- 45_performance_indexes.sql — Composite indexes untuk query hotspots
--
-- Analisis: Query di kode adalah "santri aktif" (staf_stats, points_board,
-- attendance recap) — tapi index hanya pada role, bukan role+is_active.
-- Ini menyebabkan seq scan + filter manual.
--
-- Solusi: Composite index + partial index untuk active santri.
-- Index untuk laporan discipline & attendance history.
--
-- Idempotent. Jalankan setelah migrasi 1–44.
-- =============================================================================

-- ═ 1) USERS — Query santri aktif (hampir SEMUA halaman santri) ════════════════
-- Sebelum: seq scan pada 10k+ users, filter is_active manual.
-- Sesudah: index scan → hanya active santri.
CREATE INDEX IF NOT EXISTS idx_users_active_santri
    ON users (role, is_active)
    WHERE role IN ('santri', 'santri_finance') AND is_active = TRUE;

-- ═ 2) POINT_LOGS — Laporan "Poin Pelanggaran Aktif" (30 hari terakhir) ═══════
-- Kode: WHERE delta < 0 AND category = 'discipline' AND created_at >= NOW() - INTERVAL '30 days'
-- Tanpa index: seq scan pada jutaan point_logs.
CREATE INDEX IF NOT EXISTS idx_point_logs_violations
    ON point_logs (category, created_at DESC)
    WHERE delta < 0 AND category = 'discipline';

-- ═ 3) ATTENDANCES — Riwayat santri + filter status (rekapitulasi) ═════════════
-- Sebelum: 2 index terpisah (user_id, status) atau (user_id, scanned_at).
--          Filter status manual atau sorting lambat.
-- Sesudah: Cover index (user_id, status, scanned_at DESC) → hanya 1 index scan.
CREATE INDEX IF NOT EXISTS idx_att_user_status_date
    ON attendances (user_id, status, scanned_at DESC);

-- ═ 4) PERMIT_REQUESTS — Antrean approval (workflow izin) ═════════════════════
-- Kode: antrean pamong (pamong_status='pending' AND guru_status='pending') dan
-- antrean wali kelas (guru_status='pending' AND pamong_status='approved'),
-- keduanya ORDER BY created_at ASC.
--
-- CATATAN: kolom persetujuan orang tua (`parent_status`) DIHAPUS di migrasi 46 —
-- izin kini murni akademik: pamong kelas → wali kelas. Jangan index kolom itu.
CREATE INDEX IF NOT EXISTS idx_permit_workflow
    ON permit_requests (guru_status, pamong_status, created_at);

-- ═ 5) CLASS_SCHEDULES — Jadwal aktif (record_scan, active_schedule_now) ════════
-- Migrasi 7 sudah ada idx_schedule_active, tapi verifikasi disini.
CREATE INDEX IF NOT EXISTS idx_schedule_active
    ON class_schedules (class_id, status, start_date, end_date)
    WHERE status = 'active';

-- ═ 6) POINT_LOGS — Leaderboard per minggu (weekly reward check) ════════════════
-- Kode: GROUP BY user_id, WEEK(created_at) → sort by SUM(delta) DESC
CREATE INDEX IF NOT EXISTS idx_point_logs_weekly
    ON point_logs (user_id, DATE_TRUNC('week', created_at));

-- ═ 7) BILLS — Status + santri (finance: belum bayar, lunas) ═════════════════════
-- Sebelum: seq scan pada 1000+ bills.
-- Sesudah: index scan untuk query belum/lunas.
CREATE INDEX IF NOT EXISTS idx_bills_status_user
    ON bills (status, user_id, paid_at DESC)
    WHERE deleted_at IS NULL;

-- ═ 8) ACADEMIC_USER — Progress santri per course (rapor, rekapitulasi) ═════════
-- Kode: WHERE user_id = $1 ORDER BY course_id
CREATE INDEX IF NOT EXISTS idx_academic_user_progress
    ON academic_user (user_id, curriculum_id);

-- ═ CLEANUP: Terdeteksi index redundan (optional, bisa comment kalau uncertain) ═
-- Jika idx_att_user_status_date dibuat, mungkin idx_att_user_date bisa di-drop:
-- DROP INDEX IF EXISTS idx_att_user_date;
-- Tapi verifikasi terlebih dahulu dengan EXPLAIN ANALYZE existing queries.
