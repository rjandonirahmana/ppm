-- =============================================================================
-- 93_peran_dewan_guru_khusus.sql — Dua peran dewan guru dengan tugas tambahan.
--
-- ── APA YANG DITAMBAHKAN ─────────────────────────────────────────────────────
--   dewan_guru_finance — dewan guru yang JUGA mengurus keuangan, setara
--                        `santri_finance` di sisi santri: melihat tagihan,
--                        menandai lunas, memverifikasi setoran.
--   dewan_guru_absensi — dewan guru yang JUGA boleh mengesahkan kehadiran di
--                        SESI MANA PUN, bukan hanya sesi yang ia ampu atau
--                        kelas yang ia walikan.
--
-- ── KENAPA PERAN BARU, BUKAN KOLOM PENANDA ───────────────────────────────────
-- Alternatifnya menambah kolom boolean (`urus_keuangan`, `verifikator_absensi`)
-- di `users`. Itu ditolak karena seluruh aplikasi ini sudah memutuskan
-- wewenang dari SATU kolom `role` — `role_satisfies`, `require_roles`,
-- `nav_for`, `role_home`, dan tabel petak di layar staf semuanya membacanya.
-- Menambah sumber kedua berarti tiap gerbang harus membaca dua hal dan tiap
-- gerbang yang lupa membaca yang kedua gagal DIAM-DIAM, ke arah yang salah.
--
-- Keduanya tetap dewan guru sepenuhnya: `models::role_satisfies` membuat mereka
-- memenuhi setiap gerbang yang menerima `dewan_guru`, jadi yang ditulis di sini
-- hanya TAMBAHAN wewenangnya, bukan penggantinya.
--
-- ── YANG TIDAK DILAKUKAN ─────────────────────────────────────────────────────
-- Tak ada baris yang diubah. Peran ini diberikan satu per satu lewat
-- /manajemen-user oleh admin/ketua — bukan disimpulkan dari data lama, karena
-- tak ada data yang bisa membedakan "dewan guru yang mengurus uang" dari yang
-- tidak.
--
-- Idempotent. Jalankan setelah migrasi 1–92.
-- TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh` yang membungkusnya.
-- =============================================================================

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN (
        'admin', 'ketua',
        'dewan_guru', 'dewan_guru_finance', 'dewan_guru_absensi',
        'santri', 'santri_finance', 'parent', 'penjaga'
    ));
