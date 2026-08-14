-- =============================================================================
-- 87_hapus_izin_points.sql — `class_schedules.izin_points` DIBUANG.
--
-- ── APA ISINYA ───────────────────────────────────────────────────────────────
-- Kolom itu (migrasi 28) menyimpan berapa poin dipotong ketika santri IZIN,
-- diatur per jadwal kegiatan. PRD lama menyebut angka −3 untuk KBM dan −2 untuk
-- non-KBM.
--
-- ── KENAPA DIBUANG ───────────────────────────────────────────────────────────
-- Kebijakannya sudah berubah: izin yang DISETUJUI tidak memotong poin sama
-- sekali. Santri yang mengurus izinnya justru melakukan hal yang benar —
-- menghukumnya membuat orang memilih tidak melapor, dan pondok kehilangan
-- justru informasi yang paling ia butuhkan.
--
-- Perubahan itu sudah lebih dulu terjadi di dua tempat: bidang isiannya dicabut
-- dari form jadwal, dan `models::point_rule` mengembalikan 0 untuk 'permit'
-- serta 'sick'. Yang tertinggal justru bagian yang menentukan — kolomnya masih
-- ada, dan DUA query masih membacanya:
--
--   • `repository::attendance::DELTA_SQL` — dipakai verifikasi manual DAN
--     verifikasi final otomatis;
--   • `repository::permits` saat mematerialisasi absensi dari izin yang
--     disetujui.
--
-- Artinya baris lama yang kolomnya sudah terisi TETAP memotong poin, sementara
-- UI-nya tak lagi menyediakan cara untuk melihat — apalagi mengubah — angka
-- itu. Setelan tak terlihat yang masih bekerja adalah bentuk terburuk dari
-- keduanya.
--
-- ── DAMPAK PADA DATA LAMA ────────────────────────────────────────────────────
-- Poin yang SUDAH terlanjur dipotong TIDAK dikembalikan. `point_logs` adalah
-- catatan sejarah: apa yang pernah diputuskan tetap tercatat sebagaimana
-- adanya, dan saldo `users.points` dijaga trigger dari jumlah baris-baris itu.
-- Membalikkannya berarti mengarang riwayat yang tak pernah terjadi.
--
-- Yang berubah hanya ke depan: sejak migrasi ini, izin berdelta 0.
--
-- Idempotent. Jalankan setelah migrasi 1–86, BERSAMA binary yang sudah tak
-- menyebut kolom ini. TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh`
-- yang membungkusnya.
-- =============================================================================

-- CHECK-nya lebih dulu: ia menyebut kolom yang akan dibuang (migrasi 44).
-- CHECK satu-kolom sebenarnya ikut terbuang sendiri, tapi menuliskannya di sini
-- membuat urutannya tak bergantung pada perilaku yang harus diingat.
ALTER TABLE class_schedules DROP CONSTRAINT IF EXISTS chk_izin_points_nonneg;

ALTER TABLE class_schedules DROP COLUMN IF EXISTS izin_points;

ANALYZE class_schedules;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya):
--
--   -- Kolomnya sudah hilang (harus 0 baris):
--   SELECT column_name FROM information_schema.columns
--    WHERE table_name = 'class_schedules' AND column_name = 'izin_points';
--
--   -- Tak ada lagi kolom pemotong poin izin di mana pun (harus 0 baris):
--   SELECT table_name, column_name FROM information_schema.columns
--    WHERE column_name LIKE '%izin%point%' OR column_name = 'izin_potong_poin';
--
--   -- Sejak sekarang izin berdelta 0 — periksa beberapa hari ke depan bahwa
--   -- tak ada point_logs negatif baru yang alasannya menyebut izin:
--   SELECT id, user_id, delta, reason, created_at FROM point_logs
--    WHERE delta < 0 AND reason ILIKE '%izin%'
--    ORDER BY created_at DESC LIMIT 10;
-- =============================================================================
