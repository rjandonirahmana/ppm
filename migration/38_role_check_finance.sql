-- 38_role_check_finance.sql — Perbarui CHECK users_role_check: tambah role baru
-- 'ketua' (admin + finance) & 'santri_finance' (santri + finance); buang 'teacher'
-- (sudah digabung ke 'dewan_guru' di migrasi 36).

ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;

ALTER TABLE users ADD CONSTRAINT users_role_check CHECK (
    role IN ('admin', 'ketua', 'dewan_guru', 'supervisor',
             'santri', 'santri_finance', 'parent')
);
