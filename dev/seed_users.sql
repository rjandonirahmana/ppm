-- =============================================================================
-- dev/seed_users.sql — Akun contoh SATU per peran, UNTUK PENGEMBANGAN SAJA.
--
-- ⚠️ BUKAN MIGRASI. Berkas ini dulu bernama `migration/3_seed_users.sql` dan
-- ikut dijalankan rantai migrasi — termasuk di produksi. Dipindah ke sini
-- (migrasi 70) karena tiga hal:
--
--   1. Password SEMUA akun di bawah tertulis lengkap di komentar berkas ini,
--      dan hash-nya ikut masuk ke setiap database yang menjalankan migrasi.
--   2. Ia menyisipkan role 'teacher' — DILARANG CHECK sejak migrasi 38
--      (digabung ke 'dewan_guru' di migrasi 36). Pada database BARU, rantai
--      migrasi pasti berhenti di sini. Sudah diperbaiki di bawah.
--   3. Produksi tak membutuhkannya: `service::auth::ensure_seed_admin` membuat
--      admin pertama saat tabel users kosong, dengan sandi dari env
--      ADMIN_PASSWORD, dan MENOLAK berjalan bila LEPTOS_ENV=PROD tanpa env itu.
--
-- Password SEMUA akun : Tyye9#ebv        (⚠️ jangan pernah dipakai di produksi)
--
-- Idempotent: ON CONFLICT (username) DO NOTHING.
-- Jalankan MANUAL, hanya di mesin pengembangan:
--   psql "$DATABASE_URL" -f dev/seed_users.sql
--
-- Login bisa pakai username ATAU email ATAU NIS.
-- =============================================================================

INSERT INTO users
    (username, email, nis, full_name, phone_number, role, password_hash, rfid_cards, points, address)
VALUES
    -- 1) ADMIN — redirect login → /staf
    ('admin',         'admin@ppmafm.sch.id',         NULL,
     'Administrator AFM',   '+62 858-8268-5011', 'admin',
     '$2b$10$XnKUkKbMB7Ubn/D7zlmiIOlp82zDFhOc/Odi3ebcgWTXC3IIYlVzu',
     NULL, 0, 'PPM AFM, Jl. Sawo No.33B, Pondok Cina, Depok'),

    -- 2) DEWAN GURU — redirect → /dewan-guru  (dulu ditulis 'teacher';
    --    role itu digabung ke dewan_guru di migrasi 36 & dilarang sejak 38)
    ('ustadz.ahmad',  'ustadz.ahmad@ppmafm.sch.id',  NULL,
     'Ust. Ahmad Fauzi',    '+62 812-1111-2222', 'dewan_guru',
     '$2b$10$XnKUkKbMB7Ubn/D7zlmiIOlp82zDFhOc/Odi3ebcgWTXC3IIYlVzu',
     NULL, 0, NULL),

    -- 3) SUPERVISOR (pamong) — redirect → /verifikasi-pamong
    ('pamong.budi',   'pamong.budi@ppmafm.sch.id',   NULL,
     'Budi Santoso',        '+62 812-3333-4444', 'supervisor',
     '$2b$10$XnKUkKbMB7Ubn/D7zlmiIOlp82zDFhOc/Odi3ebcgWTXC3IIYlVzu',
     NULL, 0, NULL),

    -- 4) SANTRI — redirect → /santri. Punya NIS + kartu RFID (utk uji scan)
    --    + poin awal 850 (sesuai contoh desain dashboard).
    ('santri.rizky',  'rizky@student.ppmafm.sch.id', '129038',
     'Muhammad Rizky',      '+62 812-5555-6666', 'santri',
     '$2b$10$XnKUkKbMB7Ubn/D7zlmiIOlp82zDFhOc/Odi3ebcgWTXC3IIYlVzu',
     1234567890, 850, NULL),

    -- 5) PARENT (orang tua) — redirect → /orang-tua
    ('ortu.sulaiman', 'sulaiman@gmail.com',          NULL,
     'Bpk. Sulaiman Hakim', '+62 812-7777-8888', 'parent',
     '$2b$10$XnKUkKbMB7Ubn/D7zlmiIOlp82zDFhOc/Odi3ebcgWTXC3IIYlVzu',
     NULL, 0, 'Surakarta, Jawa Tengah')
ON CONFLICT (username) DO NOTHING;

-- Hubungkan orang tua ↔ santri via parent_connections (butuh migrasi 5).
INSERT INTO parent_connections (parent_id, student_id, status, responded_at)
SELECT p.id, s.id, 'connected', NOW()
FROM users p, users s
WHERE p.username = 'ortu.sulaiman' AND s.username = 'santri.rizky'
ON CONFLICT (parent_id, student_id) DO NOTHING;
