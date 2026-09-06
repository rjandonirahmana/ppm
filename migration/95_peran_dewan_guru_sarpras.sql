-- =============================================================================
-- 95_peran_dewan_guru_sarpras.sql — Dewan guru yang mengurus sarana prasarana.
--
-- Peran ketiga dengan pola yang sama seperti migrasi 93: dewan guru SEPENUHNYA,
-- ditambah satu tugas. Di sini tugasnya mendata sarana & prasarana pondok —
-- pekerjaan yang selama ini hanya bisa dilakukan admin/ketua, padahal yang
-- benar-benar tahu kursi mana yang patah adalah orang yang mengajar di
-- ruangannya.
--
-- Wewenangnya dinyatakan SATU BARIS di `models::role_satisfies` (memenuhi
-- `dewan_guru`) ditambah namanya di `SARANA_MANAGE_ROLES`. Tak ada gerbang lain
-- yang perlu tahu peran ini ada.
--
-- Idempotent. Jalankan setelah migrasi 1–94.
-- TIDAK memuat BEGIN/COMMIT sendiri — `scripts/migrate.sh` yang membungkusnya.
-- =============================================================================

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;
ALTER TABLE users ADD CONSTRAINT users_role_check
    CHECK (role IN (
        'admin', 'ketua',
        'dewan_guru', 'dewan_guru_finance', 'dewan_guru_absensi', 'dewan_guru_sarpras',
        'santri', 'santri_finance', 'parent', 'penjaga'
    ));

