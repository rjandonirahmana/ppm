-- =============================================================================
-- 53_finish_api_key_hash.sql — Tuntaskan hashing kunci perangkat RFID.
--
-- LATAR: migrasi 43 membuat kolom `api_key_hash` lalu BERHENTI di situ — kode
-- tetap membaca `api_key` plaintext, jadi kolom itu selama ini kosong dan tak
-- memberi perlindungan apa pun. Yang ada hanyalah kolom yang menyesatkan.
--
-- Migrasi ini menyelesaikannya dalam DUA LANGKAH TERPISAH yang sengaja tidak
-- dijalankan sekaligus, supaya tak ada jendela di mana mesin absensi mati:
--
--   Langkah 1 (migrasi ini)  : siapkan kolom hash + index. Kolom plaintext
--                              MASIH ADA. Aplikasi versi baru mengisi hash-nya
--                              saat start (repository::backfill_api_key_hashes)
--                              lalu melayani lookup lewat hash.
--   Langkah 2 (MANUAL, nanti): setelah dipastikan semua perangkat berfungsi,
--                              barulah kolom plaintext di-drop. Perintahnya ada
--                              di bagian (3) — SENGAJA dikomentari.
--
-- Urutan itu penting: kalau plaintext di-drop bersamaan dengan deploy, dan
-- ternyata ada yang meleset, tak ada jalan mundur — kunci aslinya hilang dan
-- semua perangkat harus dikonfigurasi ulang satu per satu.
--
-- Idempotent. Jalankan setelah migrasi 1–52.
-- =============================================================================

-- ═ 1) Kolom hash (bila migrasi 43 belum sempat jalan) ════════════════════════
ALTER TABLE rfid_devices
    ADD COLUMN IF NOT EXISTS api_key_hash TEXT;

-- Lookup terjadi pada SETIAP tap kartu — jalur terpanas sistem. Unique sekaligus
-- menolak dua perangkat berhash sama.
CREATE UNIQUE INDEX IF NOT EXISTS uq_rfid_api_key_hash
    ON rfid_devices (api_key_hash) WHERE api_key_hash IS NOT NULL;

-- Index lama atas kolom plaintext tak lagi dipakai lookup.
DROP INDEX IF EXISTS uq_rfid_api_key;

-- ═ 2) Verifikasi SEBELUM melangkah ke bagian (3) ═════════════════════════════
-- Perangkat yang hash-nya belum terisi (backfill belum jalan / gagal):
--   SELECT id, device_name FROM rfid_devices
--    WHERE api_key_hash IS NULL AND api_key IS NOT NULL AND api_key <> '';
--
-- Harus 0. Kalau belum, jalankan ulang aplikasi (backfill jalan saat start) dan
-- periksa lognya: "Backfill kunci perangkat: N di-hash".
--
-- Lalu UJI DI LAPANGAN: tempelkan kartu di TIAP perangkat, pastikan tercatat.
-- Baru setelah semuanya terbukti jalan, lanjut ke bagian (3).

-- ═ 3) Buang plaintext — JALANKAN MANUAL, TIDAK OTOMATIS ══════════════════════
-- Sengaja dikomentari. Menghapus kolom ini TIDAK BISA dibatalkan: kunci aslinya
-- hilang selamanya, dan perangkat yang belum sempat ter-hash harus didaftarkan
-- ulang dengan kunci baru.
--
--   ALTER TABLE rfid_devices DROP COLUMN api_key;
--
-- Setelah itu, `api_key` hanya ada di dua tempat: konfigurasi firmware dan
-- catatan admin. Kunci yang terlupa TIDAK bisa dibaca balik dari database —
-- pakai tombol regenerasi untuk membuat yang baru.

COMMENT ON COLUMN rfid_devices.api_key_hash IS
    'SHA-256 hex dari api_key. Lookup saat scan membandingkan hash ini, bukan plaintext. Kunci asli TIDAK tersimpan — bila lupa, regenerasi.';
