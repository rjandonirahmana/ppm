-- =============================================================================
-- 50_unbind_gate_as_room.sql — Lepaskan jadwal yang ruangnya menunjuk GERBANG
-- UTAMA.
--
-- MASALAH:
--   `class_schedules.room_id` boleh menunjuk perangkat mana pun, termasuk yang
--   berkategori `gate_utama` (migrasi 49). Padahal tap di gerbang utama SELALU
--   diartikan keluar/masuk area pondok — tak pernah jadi absensi kelas.
--
--   Akibatnya jadwal seperti itu MUSTAHIL diabsen:
--     • tap di gerbang        → hanya toggle keluar/masuk, absensi dilewati;
--     • tap di tempat lain    → ditolak, karena bukan ruang jadwal itu.
--   Santri tak pernah bisa hadir, dan tak ada pesan error yang menjelaskan.
--
-- PERBAIKAN KODE (menyertai migrasi ini):
--   • dropdown ruang tak lagi menawarkan perangkat gerbang utama;
--   • create/update jadwal menolaknya di server.
--
-- Migrasi ini membereskan data yang terlanjur begitu.
-- Idempotent. Jalankan setelah migrasi 1-49.
-- =============================================================================

-- ═ 1) Lihat dulu — JALANKAN SEBELUM bagian (2) ══════════════════════════════
--   SELECT cs.id, cs.title, c.name AS kelas, d.device_name AS ruang_gerbang
--     FROM class_schedules cs
--     JOIN classes c ON c.id = cs.class_id
--     JOIN rfid_devices d ON d.id = cs.room_id
--    WHERE d.category = 'gate_utama';

-- ═ 2) Lepaskan ikatannya → jadwal jadi "bebas ruang" ════════════════════════
-- Dikosongkan (NULL), BUKAN dipindah ke perangkat lain: hanya admin yang tahu
-- kelas itu sebenarnya di mana. Sementara ini santri bisa absen di perangkat
-- mana pun — jauh lebih baik daripada tak bisa absen sama sekali.
UPDATE class_schedules cs
   SET room_id = NULL
  FROM rfid_devices d
 WHERE d.id = cs.room_id
   AND d.category = 'gate_utama';

-- ═ 3) Setelah ini ═══════════════════════════════════════════════════════════
-- Minta admin menetapkan ruang yang benar untuk jadwal-jadwal tsb lewat
-- halaman kelas. Jadwal yang memang boleh diabsen di mana saja cukup
-- dibiarkan kosong.
