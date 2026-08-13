-- =============================================================================
-- 84_hapus_peran_pamong.sql — PERAN PAMONG DIHAPUS. Pamong menjadi guru.
--
-- Keputusan pengurus (Ags 2026): tak ada lagi petugas bernama "pamong". Yang
-- dulu pamong kini GURU biasa, dan yang mengatur sebuah kelas adalah WALI
-- KELAS-nya sendiri. Konsekuensinya tiga hal harus dibereskan sekaligus, dan
-- ketiganya harus sepadan — kalau tidak, absensi & izin menggantung di antrean
-- yang tak punya petugas:
--
--   1. `users.role = 'supervisor'` → 'dewan_guru' (guru). Nilainya juga dibuang
--      dari CHECK supaya tak ada akun pamong baru yang bisa lahir.
--   2. `classes.verify_mode` → 'guru' untuk SEMUA kelas. Mode 'dua_tahap' dan
--      'pamong' (migrasi 62) sama-sama menunggu pamong; tanpa pamong keduanya
--      berarti absensi yang tak pernah disahkan siapa pun. Trigger
--      `trg_sync_require_pamong` (migrasi 62) ikut menurunkan
--      `require_pamong = FALSE`, jadi antrean izin tahap-1 juga kosong dengan
--      sendirinya dan izin langsung jatuh ke wali kelas.
--   3. `classes.pamong_id` yang terisi DIANGKAT jadi `wali_kelas_id` bila kelas
--      itu belum punya wali — pamong sebuah kelas memang orang yang selama ini
--      mengurusnya, dan "pamong menjadi guru" berarti ia kini wali kelasnya.
--      Tanpa langkah ini, kelas non-KBM (sholat, apel, piket) kehilangan SATU-
--      SATUNYA penanggung jawabnya begitu pamong_id dikosongkan: tak muncul di
--      "Kelas Saya" siapa pun, tak ada yang menunjuk pengisi sesi, dan absensinya
--      hanya bisa disahkan admin.
--      Karena itu CHECK `chk_classes_wali_kbm` (migrasi 65 — "wali hanya di
--      KBM") ikut DILEPAS. Aturan itu dibuat ketika kelas non-KBM masih punya
--      pamong sebagai petugasnya; sekarang wali kelas adalah satu-satunya
--      jabatan yang tersisa, jadi setiap kelas harus boleh punya.
--      PERIZINAN TIDAK BERUBAH: penyetuju izin selalu wali kelas KBM santri
--      (`repository::kelas_kbm_santri` menyaring `category = 'kbm'`), jadi wali
--      di kelas sholat/apel tak menambah satu pun tahap persetujuan.
--   4. `classes.pamong_id` & `class_sessions.pamong_id` → NULL. Kolomnya
--      SENGAJA tidak di-DROP: puluhan query masih menyebutnya lewat COALESCE,
--      dan dengan isinya NULL semua cabang itu mati sendiri. Membuangnya
--      sekarang berarti mengubah query-query itu di migrasi yang sama —
--      perubahan besar yang sulit dibalik bila keliru.
--
-- YANG TIDAK BERUBAH: baris `attendances` / `permit_requests` yang terlanjur
-- menunggu di tahap pamong TIDAK disentuh. Tak perlu: antrean tahap FINAL
-- menyaring `pamong_status <> 'rejected'` (bukan `= 'approved'`), jadi baris
-- yang menggantung di tahap-1 tetap bisa difinalkan guru/wali kelas seperti
-- biasa. Menyetujuinya massal dari migrasi justru akan memberi poin kepada
-- kehadiran yang belum pernah diperiksa manusia.
--
-- Idempotent. Jalankan setelah migrasi 1–83.
-- TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh` yang membungkusnya.
-- =============================================================================

-- ── 1) Pamong jadi guru ──────────────────────────────────────────────────────
UPDATE users SET role = 'dewan_guru' WHERE role = 'supervisor';

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN (
        'admin', 'ketua', 'dewan_guru',
        'santri', 'santri_finance', 'parent', 'penjaga'
    ));

-- ── 2) Semua kelas diverifikasi guru ─────────────────────────────────────────
UPDATE classes SET verify_mode = 'guru' WHERE verify_mode <> 'guru';
ALTER TABLE classes ALTER COLUMN verify_mode SET DEFAULT 'guru';

-- CHECK dipersempit: 'dua_tahap' & 'pamong' tak lagi bisa ditulis. Kode sudah
-- menolaknya, tapi pagar di aplikasi hanya menjaga jalur yang lewat aplikasi.
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_mode;
ALTER TABLE classes ADD CONSTRAINT chk_classes_verify_mode
    CHECK (verify_mode IN ('guru'));

-- ── 3) Pamong kelas diangkat jadi WALI KELAS ─────────────────────────────────
-- Pagarnya dilepas dulu: selama CHECK migrasi 65 masih ada, kelas non-KBM tak
-- boleh punya wali sama sekali.
ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_wali_kbm;

-- Hanya kelas yang BELUM punya wali. Kelas KBM yang sudah punya wali tetap
-- miliknya — pamong di sana adalah petugas kedua, bukan pengganti.
UPDATE classes
   SET wali_kelas_id = pamong_id
 WHERE wali_kelas_id IS NULL AND pamong_id IS NOT NULL;

-- ── 4) Penugasan pamong dikosongkan ──────────────────────────────────────────
UPDATE classes       SET pamong_id = NULL WHERE pamong_id IS NOT NULL;
UPDATE class_sessions SET pamong_id = NULL WHERE pamong_id IS NOT NULL;

ANALYZE users;
ANALYZE classes;
ANALYZE class_sessions;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya, semuanya harus 0 baris / sesuai):
--
--   SELECT count(*) FROM users WHERE role = 'supervisor';            -- 0
--   SELECT count(*) FROM classes WHERE verify_mode <> 'guru';        -- 0
--   SELECT count(*) FROM classes WHERE require_pamong;               -- 0 (trigger)
--   SELECT count(*) FROM classes WHERE pamong_id IS NOT NULL;        -- 0
--   SELECT count(*) FROM class_sessions WHERE pamong_id IS NOT NULL; -- 0
--
--   -- Kelas KBM yang belum punya wali kelas = kelas tanpa penyetuju izin.
--   -- Isi walinya lewat /kelas/:id sebelum santri mengajukan izin berikutnya:
--   SELECT id, name FROM classes WHERE category = 'kbm' AND wali_kelas_id IS NULL;
--
--   -- Kelas mana pun yang kini tanpa penanggung jawab (mis. dulu tak berpamong):
--   SELECT id, name, category FROM classes WHERE wali_kelas_id IS NULL ORDER BY category, name;
--
--   -- Siapa saja yang naik dari pamong jadi wali (jalankan SEBELUM migrasi):
--   SELECT c.id, c.name, c.category, u.full_name AS calon_wali
--     FROM classes c JOIN users u ON u.id = c.pamong_id
--    WHERE c.wali_kelas_id IS NULL;
-- =============================================================================
