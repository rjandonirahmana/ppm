-- =============================================================================
-- 80_index_laporan.sql — Penopang query halaman /laporan.
--
-- Halaman laporan menembakkan enam query sekaligus (sudah paralel lewat
-- `tokio::join!`), jadi yang dirasakan pengguna adalah query TERLAMBAT di antara
-- keenamnya — bukan jumlahnya. Dua di antaranya sama sekali tak punya index
-- yang bisa dipakai.
--
-- ── 1. `point_logs (created_at DESC)` ────────────────────────────────────────
-- `recent_points_all` bertanya "enam catatan poin TERBARU dari seluruh santri":
--
--     SELECT … FROM point_logs p JOIN users u ON u.id = p.user_id
--      WHERE u.role IN ('santri','santri_finance')
--      ORDER BY p.created_at DESC LIMIT 6
--
-- Index `point_logs` yang ada semuanya berawalan kolom lain — `(user_id,
-- created_at DESC)` dan `(category, created_at DESC)`. Keduanya sangat baik
-- untuk "riwayat poin SI A" atau "pelanggaran 30 hari terakhir", dan tak satu
-- pun bisa menjawab "yang terbaru dari SIAPA SAJA": tanpa nilai kolom paling
-- kiri, btree tak bisa ditelusuri.
--
-- Akibatnya Postgres mengurutkan SELURUH `point_logs` hanya untuk mengambil
-- enam baris teratas. Tabel itu bertambah satu baris setiap absensi
-- diverifikasi — ia tumbuh paling cepat di antara semua tabel di sini, dan
-- biayanya bertambah tiap hari sementara jumlah baris yang ditampilkan tetap
-- enam.
--
-- ── 2. `attendances (scanned_at DESC)` ───────────────────────────────────────
-- `analisis_summary` dan `class_ranking` sama-sama menyaring rentang 30 hari
-- pada `scanned_at` tanpa menyebut `user_id`. Index yang ada, `(user_id,
-- scanned_at DESC)`, punya persoalan yang sama seperti di atas.
--
-- Ini index KETIGA pada tabel yang sama untuk sumbu waktu (`scan_date`,
-- `pamong_at`/`verified_at` parsial di migrasi 79, dan sekarang `scanned_at`),
-- jadi biayanya nyata: tiap INSERT absensi memperbarui satu btree lagi. Tetap
-- sepadan karena tabel ini dibaca jauh lebih sering daripada ditulis — satu tap
-- kartu per santri per sesi, melawan dashboard yang dibuka berkali-kali sehari
-- oleh pamong, guru, dewan guru, dan admin.
--
-- ── 3. Konfirmasi sebelum percaya ────────────────────────────────────────────
-- Index adalah tebakan tentang rencana eksekusi sampai dibuktikan. Jalankan
-- EXPLAIN di bagian verifikasi bawah setelah migrasi ini. Bila planner masih
-- memilih seq scan, itu wajar untuk tabel yang belum besar — index-nya baru
-- terpakai saat datanya bertambah, dan tak merugikan sementara itu.
--
-- Idempotent. Jalankan setelah migrasi 1–79.
-- =============================================================================

CREATE INDEX IF NOT EXISTS idx_point_logs_terbaru
    ON point_logs (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_att_scanned_terbaru
    ON attendances (scanned_at DESC);

ANALYZE point_logs;
ANALYZE attendances;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   EXPLAIN (ANALYZE, BUFFERS)
--   SELECT p.reason, p.delta FROM point_logs p
--     JOIN users u ON u.id = p.user_id
--    WHERE u.role IN ('santri','santri_finance')
--    ORDER BY p.created_at DESC LIMIT 6;
--   -- Diharapkan: Index Scan Backward pakai idx_point_logs_terbaru,
--   -- BUKAN "Sort" atas seluruh tabel.
--
--   EXPLAIN (ANALYZE, BUFFERS)
--   SELECT COUNT(*) FROM attendances a JOIN users u ON u.id = a.user_id
--    WHERE u.role IN ('santri','santri_finance')
--      AND a.scanned_at >= NOW() - INTERVAL '30 days';
--
--   -- Setelah beberapa hari, index yang tak pernah tersentuh:
--   SELECT indexrelname, idx_scan FROM pg_stat_user_indexes
--    WHERE relname IN ('point_logs','attendances') ORDER BY idx_scan;
-- =============================================================================
