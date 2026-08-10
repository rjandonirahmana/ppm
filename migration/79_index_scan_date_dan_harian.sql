-- =============================================================================
-- 79_index_scan_date_dan_harian.sql — Index untuk pertanyaan "hari itu" dan
-- "hari ini", dua bentuk yang paling sering ditanyakan dashboard.
--
-- ── 1. Kenapa `scan_date` sendirian ──────────────────────────────────────────
-- Tabel `attendances` sudah punya banyak index, tapi TAK SATU PUN berawalan
-- `scan_date`. Yang ada semuanya menaruhnya di posisi kedua:
--   (user_id, scan_date)                  — migrasi 68 & 70
--   (class_schedule_id, scan_date)        — migrasi 68
--   (user_id, class_schedule_id, scan_date)
--
-- Btree hanya bisa dipakai bila kolom PALING KIRI ikut disaring. Semua index di
-- atas menjawab "absensi si A tanggal itu" dengan sangat baik, dan tak satu pun
-- bisa menjawab "SEMUA absensi tanggal itu" — pertanyaan yang justru dipakai
-- setiap dashboard: tren 7 hari, statistik staf, rekap mingguan, dan hitung
-- per kategori. Semuanya berakhir memindai seluruh tabel, pada tabel yang
-- diproyeksikan tumbuh ±1 juta baris per tahun (catatan migrasi 68).
--
-- ── 2. Kenapa dua index parsial "hari ini" ───────────────────────────────────
-- Kartu "disetujui hari ini" / "diverifikasi hari ini" menyaring DUA hal
-- sekaligus: statusnya, dan waktunya jatuh hari ini. Kode-nya baru saja diubah
-- dari `(kolom AT TIME ZONE …)::date = …` menjadi perbandingan RENTANG
-- (`repository::hari_ini_wib`) — bentuk lama membungkus kolomnya dalam fungsi
-- sehingga index apa pun mustahil dipakai, dan index ekspresi juga mustahil
-- dibuat karena ekspresinya STABLE, bukan IMMUTABLE.
--
-- Sekarang bentuknya sudah bisa memakai index, index-nya tinggal disediakan.
-- Dibuat PARSIAL karena predikat statusnya selalu ikut: hanya baris 'approved'
-- yang pernah dihitung, dan itu sebagian kecil dari tabel — index parsial
-- membuatnya kecil sekaligus tetap tepat sasaran.
--
-- ── 3. Konfirmasi sebelum percaya ────────────────────────────────────────────
-- Index adalah tebakan tentang rencana eksekusi sampai dibuktikan. Setelah
-- migrasi ini dijalankan di VPS, jalankan EXPLAIN (ANALYZE, BUFFERS) untuk
-- query nyata di bagian verifikasi bawah dan pastikan planner benar-benar
-- memakainya. Bila ternyata seq scan tetap menang (tabel masih kecil, itu
-- normal), index-nya tak merugikan — ia baru terpakai saat datanya bertambah.
--
-- Idempotent. Jalankan setelah migrasi 1–78.
-- =============================================================================

-- ── 1. Semua absensi pada satu tanggal ───────────────────────────────────────
-- Nama BARU yang tak menyerupai index mana pun yang sudah ada: `IF NOT EXISTS`
-- mencocokkan NAMA, bukan definisi — memakai ulang nama lama membuat migrasi
-- terlihat sukses padahal indexnya tak pernah terbuat (jebakan yang sudah
-- menggigit di migrasi 45, 70, dan 75).
CREATE INDEX IF NOT EXISTS idx_att_tanggal_saja
    ON attendances (scan_date);

-- ── 2. Hitungan "hari ini" per tahap verifikasi ──────────────────────────────
CREATE INDEX IF NOT EXISTS idx_att_pamong_disetujui_waktu
    ON attendances (pamong_at) WHERE pamong_status = 'approved';

CREATE INDEX IF NOT EXISTS idx_att_final_disetujui_waktu
    ON attendances (verified_at) WHERE verify_status = 'approved';

-- ── 3. Keputusan izin "hari ini" ─────────────────────────────────────────────
-- Pasangan dari dua di atas untuk tabel izin: kartu "izin diputus hari ini" di
-- dashboard pamong dan wali kelas. Parsial `<> 'pending'` mengikuti predikat
-- query-nya — baris yang masih menunggu tak pernah masuk hitungan ini.
CREATE INDEX IF NOT EXISTS idx_permit_pamong_diputus_waktu
    ON permit_requests (pamong_at) WHERE pamong_status <> 'pending';

CREATE INDEX IF NOT EXISTS idx_permit_guru_diputus_waktu
    ON permit_requests (guru_at) WHERE guru_status <> 'pending';

ANALYZE attendances;
ANALYZE permit_requests;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   -- Bandingkan DEFINISI, bukan sekadar nama:
--   SELECT indexrelid::regclass, pg_get_indexdef(indexrelid)
--     FROM pg_index WHERE indrelid = 'attendances'::regclass;
--   SELECT indexrelid::regclass, pg_get_indexdef(indexrelid)
--     FROM pg_index WHERE indrelid = 'permit_requests'::regclass;
--
--   -- Rencana eksekusi nyata (lihat catatan 3 di atas):
--   EXPLAIN (ANALYZE, BUFFERS)
--   SELECT COUNT(*) FROM attendances
--    WHERE verify_status = 'approved'
--      AND verified_at >= (date_trunc('day', NOW() AT TIME ZONE 'Asia/Jakarta')
--                          AT TIME ZONE 'Asia/Jakarta')
--      AND verified_at <  (date_trunc('day', NOW() AT TIME ZONE 'Asia/Jakarta')
--                          AT TIME ZONE 'Asia/Jakarta') + INTERVAL '1 day';
--
--   EXPLAIN (ANALYZE, BUFFERS)
--   SELECT COUNT(*) FROM attendances WHERE scan_date = CURRENT_DATE;
--
--   -- Setelah beberapa hari berjalan, index yang tak pernah tersentuh:
--   SELECT indexrelname, idx_scan FROM pg_stat_user_indexes
--    WHERE relname IN ('attendances','permit_requests') ORDER BY idx_scan;
-- =============================================================================
