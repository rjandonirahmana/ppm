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
-- NAMA BEDA dari idx_schedule_active (migrasi 7). Dengan nama yang sama,
-- IF NOT EXISTS akan melewati pembuatan DIAM-DIAM — index parsial di bawah tak
-- pernah terbuat, dan tak ada yang sadar karena migrasi tetap "sukses".
CREATE INDEX IF NOT EXISTS idx_schedule_active_partial
    ON class_schedules (class_id, start_date, end_date)
    WHERE status = 'active';

-- ═ 6) POINT_LOGS — rentang waktu per santri ═══════════════════════════════════
-- SENGAJA index kolom mentah, BUKAN DATE_TRUNC('week', created_at):
-- date_trunc atas timestamptz bergantung setelan TimeZone sesi → STABLE, bukan
-- IMMUTABLE, dan Postgres MENOLAKNYA di dalam index (migrasi 7 sudah mencatat
-- jebakan yang sama untuk `AT TIME ZONE`). Versi awal migrasi ini memakainya →
-- seluruh rantai migrasi gagal di sini.
--
-- Query mingguan menyaring rentang `created_at BETWEEN ... AND ...`, jadi index
-- kolom mentah ini justru yang terpakai.
CREATE INDEX IF NOT EXISTS idx_point_logs_user_created
    ON point_logs (user_id, created_at DESC);

-- ═ 7) BILLS — Status + santri (finance: belum bayar, lunas) ═════════════════════
-- Sebelum: seq scan pada 1000+ bills.
-- Sesudah: index scan untuk query belum/lunas.
CREATE INDEX IF NOT EXISTS idx_bills_status_user
    ON bills (status, user_id, paid_at DESC)
    WHERE deleted_at IS NULL;

-- ═ 8) ACADEMIC_USER — Progres materi per santri ═══════════════════════════════
-- CATATAN: tabel ini berkolom `book_id` (migrasi 18), BUKAN `curriculum_id`.
-- Versi awal migrasi ini menyebut curriculum_id → seluruh rantai migrasi gagal.
--
-- Migrasi 18 SUDAH membuat idx_academic_user_user (user_id) dan
-- idx_academic_user_book (book_id), plus UNIQUE (user_id, book_id) yang sudah
-- berfungsi sebagai index komposit. Jadi tak ada index tambahan yang perlu —
-- bagian ini sengaja dikosongkan agar tak ada index kembar yang percuma.

-- ═ CLEANUP: Terdeteksi index redundan (optional, bisa comment kalau uncertain) ═
-- Jika idx_att_user_status_date dibuat, mungkin idx_att_user_date bisa di-drop:
-- DROP INDEX IF EXISTS idx_att_user_date;
-- Tapi verifikasi terlebih dahulu dengan EXPLAIN ANALYZE existing queries.
