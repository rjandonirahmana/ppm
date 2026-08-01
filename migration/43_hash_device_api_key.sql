-- =============================================================================
-- 43_hash_device_api_key.sql — Hash rfid_devices.api_key (security fix)
--
-- Masalah: API key perangkat RFID disimpan plaintext di database.
-- Perbaikan: Ubah ke SHA-256 hash. Komparasi di aplikasi akan gunakan hash.
--
-- NOTE: Manual backfill di aplikasi setelah migrasi ini. Query SELECT
-- akan berubah ke WHERE api_key_hash = encode(digest($1, 'sha256'), 'hex')
--
-- Idempotent. Jalankan setelah migrasi 1–42.
-- =============================================================================

-- 1) Buat kolom baru untuk menyimpan hash (sebelum hapus plaintext).
ALTER TABLE rfid_devices
    ADD COLUMN IF NOT EXISTS api_key_hash TEXT;

-- 2) Buat unique index untuk hash (optional, tapi recommended).
CREATE UNIQUE INDEX IF NOT EXISTS uq_device_api_key_hash
    ON rfid_devices (api_key_hash)
    WHERE api_key_hash IS NOT NULL;

-- 3) CATATAN: Backfill api_key → api_key_hash harus dilakukan dengan hati-hati.
--    Opsi a) Manual di aplikasi (safe):
--      - Aplikasi baca api_key plaintext
--      - Hash dengan SHA-256 di Rust
--      - Simpan ke api_key_hash
--      - Lalu: ALTER TABLE rfid_devices DROP COLUMN api_key
--    Opsi b) Direct SQL (jika database ada pgcrypto):
--      UPDATE rfid_devices SET api_key_hash = encode(digest(api_key, 'sha256'), 'hex')
--      WHERE api_key_hash IS NULL;
--
--    Untuk saat ini: hanya buat kolom. Backfill & drop plaintext di step terpisah.
