-- =============================================================================
-- 9_cleanup_orphan_sessions.sql — Bersihkan SESI YATIM (tanpa jadwal).
--
-- Sesi yatim = class_sessions.class_schedule_id IS NULL → jadwalnya sudah dihapus
-- (delete_schedule me-NULL-kan sesi) ATAU sesi dibuat manual tanpa jadwal.
-- Muncul di UI (ada sesi tapi tab Jadwal kosong).
--
-- HATI-HATI FK:
--   • attendances.class_session_id  → ON DELETE SET NULL (absensi selamat)
--   • class_session_chats.session_id → ON DELETE CASCADE  (CHAT IKUT TERHAPUS)
-- Maka versi default HANYA menghapus yatim yang BELUM ada absensi & chat.
--
-- Jalankan: psql "$DATABASE_URL" -f migration/9_cleanup_orphan_sessions.sql
-- =============================================================================

-- 1) PREVIEW dulu (tak menghapus apa pun) — lihat apa yang akan kena.
SELECT c.name AS kelas, cs.id, cs.title, cs.session_date, cs.status,
       EXISTS (SELECT 1 FROM attendances a       WHERE a.class_session_id = cs.id) AS ada_absensi,
       EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id   = cs.id) AS ada_chat
FROM class_sessions cs
JOIN classes c ON c.id = cs.class_id
WHERE cs.class_schedule_id IS NULL
ORDER BY c.name, cs.session_date DESC;

-- 2) HAPUS sesi yatim yang AMAN (belum ada absensi & chat) — semua kelas.
DELETE FROM class_sessions cs
WHERE cs.class_schedule_id IS NULL
  AND NOT EXISTS (SELECT 1 FROM attendances a       WHERE a.class_session_id = cs.id)
  AND NOT EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id   = cs.id);
