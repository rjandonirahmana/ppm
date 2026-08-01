-- =============================================================================
-- 47_santri_registration_profile.sql — Profil mahasiswa jadi bagian REGISTRASI.
--
-- Sebelumnya gender/campus/major/entry_year hanya bisa diisi santri sendiri di
-- halaman profil SETELAH akun jadi — banyak yang tak pernah diisi. Sekarang
-- keempatnya WAJIB saat mendaftar dengan peran santri / santri_finance.
--
-- PERUBAHAN MAKNA `entry_year`:
--   Migrasi 39 mendefinisikannya sebagai "tahun masuk KULIAH". Mulai sekarang
--   maknanya "tahun masuk PPM" — itu yang dipakai pengurus untuk angkatan
--   pondok. Baris LAMA yang terlanjur diisi sebagai tahun masuk kuliah akan
--   terbaca sebagai tahun masuk PPM; tak ada cara otomatis membedakannya.
--
--   Kalau ada data lama yang perlu dikoreksi, jalankan verifikasi di bagian (3)
--   dan perbaiki manual SEBELUM santri melihat angka yang salah di profilnya.
--
-- Kolom sudah ada semua (migrasi 26: campus/major/gender; migrasi 39:
-- entry_year) — migrasi ini hanya menambah dokumentasi & batas nilai.
-- Idempotent. Jalankan setelah migrasi 1–46.
-- =============================================================================

-- ═ 1) Dokumentasikan makna kolom di database itu sendiri ═════════════════════
-- Supaya siapa pun yang membaca skema langsung tahu, tanpa menelusuri migrasi.
COMMENT ON COLUMN users.entry_year IS
    'Tahun masuk PPM (bukan tahun masuk kuliah). Wajib saat registrasi santri. Lihat migrasi 47.';
COMMENT ON COLUMN users.campus IS 'Nama kampus santri. Wajib saat registrasi santri.';
COMMENT ON COLUMN users.major  IS 'Jurusan/program studi santri. Wajib saat registrasi santri.';
COMMENT ON COLUMN users.gender IS 'L = laki-laki, P = perempuan. Wajib saat registrasi santri.';

-- ═ 2) Batas nilai tahun masuk ════════════════════════════════════════════════
-- Rentang longgar (pondok berdiri jauh sebelum aplikasi ini) tapi cukup untuk
-- menangkap salah ketik seperti 202 atau 20245.
ALTER TABLE users DROP CONSTRAINT IF EXISTS chk_users_entry_year;
ALTER TABLE users ADD CONSTRAINT chk_users_entry_year
    CHECK (entry_year IS NULL OR entry_year BETWEEN 1990 AND 2100);

-- CATATAN: kolomnya TIDAK dibuat NOT NULL. Kewajiban mengisi ditegakkan di
-- lapisan registrasi (service/registration.rs), bukan di skema — sebab peran
-- lain (guru, pamong, orang tua, admin) memang tak punya data ini, dan akun
-- santri lama yang dibuat sebelum aturan ini belum tentu terisi.

-- ═ 3) Verifikasi — jalankan MANUAL di staging sebelum produksi ═══════════════
-- Santri yang entry_year-nya kemungkinan masih bermakna "tahun masuk kuliah"
-- (diisi sebelum migrasi ini). Periksa dan koreksi manual bila perlu:
--   SELECT id, full_name, nis, campus, entry_year
--     FROM users
--    WHERE role IN ('santri','santri_finance') AND entry_year IS NOT NULL
--    ORDER BY entry_year;
--
-- Santri lama yang datanya belum lengkap — perlu diminta melengkapi lewat
-- halaman profil:
--   SELECT id, full_name, nis
--     FROM users
--    WHERE role IN ('santri','santri_finance') AND is_active
--      AND (gender IS NULL OR campus IS NULL OR major IS NULL OR entry_year IS NULL);
