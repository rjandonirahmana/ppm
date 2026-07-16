-- =============================================================================
-- 3_seed_users.sql — Seed SATU user per role (untuk pengujian).
--
-- Password SEMUA akun : Tyye9#ebv        (⚠️ ganti di produksi!)
-- Hash bcrypt sudah DIVERIFIKASI cocok (cargo run -- verify ... → COCOK).
--
-- Idempotent: ON CONFLICT (username) DO NOTHING — aman dijalankan berulang.
-- Jalankan SETELAH 1.sql + 2.sql:
--   psql "$DATABASE_URL" -f migration/3_seed_users.sql
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

    -- 2) TEACHER (dewan guru) — redirect → /guru
    ('ustadz.ahmad',  'ustadz.ahmad@ppmafm.sch.id',  NULL,
     'Ust. Ahmad Fauzi',    '+62 812-1111-2222', 'teacher',
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
