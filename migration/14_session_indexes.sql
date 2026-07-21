-- =============================================================================
-- 14_session_indexes.sql — Index kecil yang belum ada di class_sessions.
-- Murah dipasang, nol risiko regresi (CREATE INDEX additive) — dipasang
-- sekarang meski skala saat ini (~92 santri) belum butuh, krn kapan pun
-- dipasang efeknya sama & tak ada downside menunggu.
--
-- session_date: dipakai WHERE di HAMPIR semua query daftar sesi (all_sessions,
-- sessions_for_student, sessions_of_class — repository/schedule.rs) + ORDER BY.
-- teacher_id: dipakai analisis_summary/attendance_trend_7d/class_ranking saat
-- teacher_id=Some(tid) (dashboard guru individual — repository/kelas.rs).
--
-- Idempotent. Jalankan setelah migrasi 1–13.
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_sessions_date ON class_sessions (session_date);
CREATE INDEX IF NOT EXISTS idx_sessions_teacher ON class_sessions (teacher_id)
    WHERE teacher_id IS NOT NULL;
