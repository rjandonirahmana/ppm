-- =============================================================================
-- 64_permit_request_classes.sql — Izin bisa mencakup BANYAK kelas.
--
-- KENAPA
-- Satu pengajuan izin dipecah per WALI KELAS (migrasi 46): kelas A & B yang
-- walinya sama cukup diputus sekali. Barisnya sudah benar — yang salah adalah
-- apa yang tersimpan. `permit_requests.class_id` hanya muat SATU kelas, jadi
-- yang dicatat kelas PERTAMA grup itu saja.
--
-- Akibatnya izin yang di layar berbunyi "berlaku untuk kelas A dan B" hanya
-- benar-benar berlaku untuk A:
--   • materialize_permit_attendance() menyisipkan baris izin hanya untuk sesi
--     `s.class_id = p.class_id`  → sesi kelas B tak dapat baris izin;
--   • run_auto_absent() mengecualikan izin dengan syarat yang sama → santri
--     yang izinnya SUDAH disetujui tetap di-ALPA-kan otomatis di kelas B,
--     lengkap dengan potongan poinnya.
-- Tak ada galat, tak ada log; santrinya yang menanggung.
--
-- YANG DILAKUKAN
-- Tabel penghubung permit → kelas. `permit_requests.class_id` DIPERTAHANKAN
-- sebagai kelas ACUAN PERSETUJUAN (penentu require_pamong & pamong penanggung
-- jawab, dipakai decide_guru_permit) — perannya berbeda dari "kelas mana saja
-- yang dicakup izin ini", dan menggabungkan dua pertanyaan berbeda ke satu
-- kolom itulah asal bugnya.
--
-- Baris lama di-backfill dari class_id-nya: cakupannya tak jadi lebih benar
-- (data kelas lain sudah tak tersimpan), tapi setidaknya tak jadi lebih buruk,
-- dan query baru tak perlu mengurus dua bentuk data.
--
-- Idempotent. Jalankan setelah migrasi 1–63.
-- =============================================================================

CREATE TABLE IF NOT EXISTS permit_request_classes (
    permit_id BIGINT NOT NULL REFERENCES permit_requests(id) ON DELETE CASCADE,
    class_id  BIGINT NOT NULL REFERENCES classes(id)         ON DELETE CASCADE,
    PRIMARY KEY (permit_id, class_id)
);

-- Arah baca kedua: "sesi kelas ini, adakah izin yang mencakupnya?" — dipakai
-- run_auto_absent yang berjalan tiap beberapa menit untuk SELURUH kelas.
CREATE INDEX IF NOT EXISTS idx_prc_class ON permit_request_classes (class_id);

INSERT INTO permit_request_classes (permit_id, class_id)
SELECT id, class_id FROM permit_requests WHERE class_id IS NOT NULL
ON CONFLICT DO NOTHING;

-- Verifikasi:
--   SELECT p.id, p.class_id, array_agg(prc.class_id) AS cakupan
--     FROM permit_requests p
--     LEFT JOIN permit_request_classes prc ON prc.permit_id = p.id
--    GROUP BY p.id ORDER BY p.id DESC LIMIT 20;
