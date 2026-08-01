-- =============================================================================
-- 49_rfid_device_category.sql — Kategori perangkat RFID + kunci numerik.
--
-- LATAR:
--   Sebelumnya perilaku scan ditentukan oleh ENDPOINT yang dipanggil firmware:
--   /api/rfid/scan → absensi kelas, /api/rfid/gate → gerbang pondok. Artinya
--   satu perangkat harus di-flash ulang untuk berganti peran, dan salah pasang
--   firmware = absensi kelas tercatat padahal santri cuma lewat gerbang.
--
--   Sekarang PERANGKAT yang menentukan perilakunya, lewat kolom `category`.
--   Firmware cukup satu macam (selalu POST /api/rfid/scan); server yang
--   merutekan berdasarkan kategori perangkat pemilik api_key.
--
-- KATEGORI:
--   gate_utama    → tap = KELUAR/MASUK area PPM (toggle), BUKAN absensi kelas.
--                   Inilah penanda santri sedang di dalam atau di luar pondok.
--   gedung_putra  ┐
--   gedung_putri  ├→ absensi kelas biasa (cocokkan jadwal aktif santri).
--   masjid        ┘
--   custom        → absensi kelas biasa; untuk lokasi yang tak masuk kategori
--                   di atas.
--
-- CATATAN JADWAL: pencocokan jadwal (`active_schedule_now`) TIDAK menyaring
--   perangkat — jadwal kelas bisa di-tap di perangkat mana pun selain
--   gate_utama. Kolom `class_schedules.room_id` hanya keterangan ruang.
--
-- Idempotent. Jalankan setelah migrasi 1–48.
-- =============================================================================

-- ═ 1) Kolom kategori ═════════════════════════════════════════════════════════
-- Default 'custom' supaya perangkat lama tetap berperilaku seperti sekarang
-- (absensi kelas). Admin menandai mana yang gerbang utama lewat UI.
ALTER TABLE rfid_devices
    ADD COLUMN IF NOT EXISTS category VARCHAR(20) NOT NULL DEFAULT 'custom';

ALTER TABLE rfid_devices DROP CONSTRAINT IF EXISTS chk_rfid_category;
ALTER TABLE rfid_devices ADD CONSTRAINT chk_rfid_category
    CHECK (category IN ('gate_utama', 'gedung_putra', 'gedung_putri', 'masjid', 'custom'));

-- ═ 2) api_key unik ═══════════════════════════════════════════════════════════
-- Kunci kini digit-saja (lebih mudah diketik di captive portal firmware).
-- Ruang nilainya lebih kecil dari hex, jadi tabrakan harus ditolak DB — jangan
-- hanya diandalkan ke generator.
CREATE UNIQUE INDEX IF NOT EXISTS uq_rfid_api_key ON rfid_devices (api_key);

-- ═ 3) Lookup saat scan ═══════════════════════════════════════════════════════
-- find_device_by_key dipanggil pada SETIAP tap kartu — jalur terpanas sistem.
-- (Sudah tercakup uq_rfid_api_key di atas; index terpisah tak perlu.)

-- ═ 4) Tandai perangkat gerbang yang sudah ada — JALANKAN MANUAL ═════════════
-- Lihat dulu kandidatnya:
--   SELECT id, device_name, location, category FROM rfid_devices ORDER BY id;
--
-- Lalu tandai yang memang gerbang utama, mis.:
--   UPDATE rfid_devices SET category = 'gate_utama' WHERE id IN (...);
--
-- SELAMA belum ditandai, perangkat gerbang lama TETAP BERFUNGSI lewat endpoint
-- /api/rfid/gate (dipertahankan demi firmware yang belum diperbarui). Yang
-- berubah hanya: /api/rfid/scan kini ikut merutekan bila kategorinya gate_utama.
