-- =============================================================================
-- 19_phone_unique.sql — Nomor HP unik (registrasi via link undangan, migrasi
-- berikutnya di kode: service/registration.rs).
--
-- users.phone_number TIDAK PERNAH punya UNIQUE/index di 18 migrasi sebelumnya
-- (cuma username/email/nis). Alur registrasi baru pakai phone_number sebagai
-- kunci login (bukan username/email/nis, yang tetap diisi admin belakangan) —
-- wajib unik supaya tak ada dua akun bentrok nomor HP yang sama.
--
-- Partial (WHERE phone_number IS NOT NULL) — pola sama uq_cp_one_primary
-- (migrasi 2): banyak akun lama boleh punya phone_number NULL/kosong.
--
-- CATATAN: bila di DB production sudah ada NOMOR HP DUPLIKAT (bukan NULL),
-- migrasi ini akan GAGAL diterapkan — itu sinyal utk membersihkan data dulu,
-- bukan sesuatu yang diam-diam dilewati di sini.
--
-- Idempotent. Jalankan setelah migrasi 1–18.
-- =============================================================================

CREATE UNIQUE INDEX IF NOT EXISTS uq_users_phone ON users (phone_number) WHERE phone_number IS NOT NULL;
