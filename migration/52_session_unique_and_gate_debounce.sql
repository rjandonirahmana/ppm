-- =============================================================================
-- 52_session_unique_and_gate_debounce.sql — Dua celah balapan (race).
--
-- (A) DUPLIKAT SESI
--     `insert_sessions` memakai `WHERE NOT EXISTS (...)` untuk idempotensi.
--     Itu TIDAK atomik: dua materialisasi yang berjalan bersamaan (job latar +
--     `update_schedule` yang dipicu admin) sama-sama membaca "belum ada", lalu
--     sama-sama menyisipkan. Hasilnya dua sesi untuk jadwal & tanggal yang sama
--     → absensi terpecah, `session_for_schedule_today` (LIMIT 1 tanpa ORDER BY)
--     memilih salah satunya sembarangan.
--
-- (B) TAP GERBANG BERULANG
--     Ditangani di kode (repository::toggle_gate), bukan di sini — lihat
--     bagian (2) untuk kolom yang dibutuhkannya.
--
-- Idempotent. Jalankan setelah migrasi 1–51.
-- =============================================================================

-- ═ 1A) Bereskan duplikat LAMA sebelum memasang UNIQUE ════════════════════════
-- HANYA menghapus duplikat yang BENAR-BENAR kosong: tak punya absensi maupun
-- chat. Sesi yang sudah dipakai TIDAK disentuh — menghapusnya berarti membuang
-- catatan kehadiran santri, dan itu jauh lebih buruk daripada membiarkan satu
-- duplikat. Yang tersisa (bila ada) dilaporkan di bagian (3) untuk dibereskan
-- manusia.
DELETE FROM class_sessions s
 WHERE s.class_schedule_id IS NOT NULL
   AND EXISTS (
         SELECT 1 FROM class_sessions t
          WHERE t.class_schedule_id = s.class_schedule_id
            AND t.session_date = s.session_date
            AND t.id < s.id                       -- simpan yang TERTUA
       )
   AND NOT EXISTS (SELECT 1 FROM attendances a WHERE a.class_session_id = s.id)
   AND NOT EXISTS (SELECT 1 FROM class_session_chats ch WHERE ch.session_id = s.id);

-- ═ 1B) Cegah duplikat baru ═══════════════════════════════════════════════════
-- Parsial: sesi ad-hoc (class_schedule_id NULL) memang boleh banyak dalam satu
-- hari untuk kelas yang sama — itu bukan duplikat, itu pertemuan tambahan.
--
-- CATATAN: bila pemasangan index ini GAGAL, berarti masih ada duplikat yang
-- punya absensi/chat. Jalankan query di bagian (3), gabungkan datanya manual,
-- lalu ulangi migrasi ini.
CREATE UNIQUE INDEX IF NOT EXISTS uq_session_schedule_date
    ON class_sessions (class_schedule_id, session_date)
    WHERE class_schedule_id IS NOT NULL;

-- ═ 2) Kolom untuk debounce gerbang ═══════════════════════════════════════════
-- `toggle_gate` membalik status setiap tap. Kartu yang memantul di pembaca
-- (atau ditahan sebentar) menghasilkan dua tap dalam sedetik → keluar lalu
-- masuk lagi → status akhir SALAH, dan riwayatnya berisi dua baris palsu.
--
-- `gate_at` sudah ada (migrasi gerbang) dan menyimpan waktu tap terakhir; kode
-- akan memakainya sebagai jendela abai. Tak ada kolom baru yang dibutuhkan —
-- bagian ini sengaja hanya dokumentasi supaya jejak keputusannya ada.

-- ═ 3) Verifikasi — JALANKAN MANUAL di staging ════════════════════════════════
-- Duplikat yang TERSISA (punya data, jadi tak dihapus otomatis):
--   SELECT class_schedule_id, session_date, COUNT(*), array_agg(id ORDER BY id)
--     FROM class_sessions
--    WHERE class_schedule_id IS NOT NULL
--    GROUP BY class_schedule_id, session_date
--   HAVING COUNT(*) > 1;
--
-- Untuk tiap grup: pindahkan absensi & chat ke sesi tertua, lalu hapus sisanya.
--   UPDATE attendances SET class_session_id = <id_tertua> WHERE class_session_id = <id_lain>;
--   UPDATE class_session_chats SET session_id = <id_tertua> WHERE session_id = <id_lain>;
--   DELETE FROM class_sessions WHERE id = <id_lain>;
