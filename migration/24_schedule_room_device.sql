-- =============================================================================
-- 24_schedule_room_device.sql — "Ruang" jadwal jadi REFERENSI perangkat RFID.
-- Dulu class_schedules.room = teks bebas; kini `room_id` → rfid_devices.id,
-- karena tiap ruang punya scanner RFID (santri masuk via kartu RFID). Buat
-- perangkat RFID dulu (User Control) sebelum pilih ruang di jadwal.
--
-- Kolom lama `room` DIBIARKAN (tak dipakai kode lagi) agar migrasi non-destruktif.
-- ON DELETE SET NULL: perangkat dihapus → jadwal kehilangan ruang, tak terhapus.
-- Idempotent. Jalankan setelah migrasi 1–23.
-- =============================================================================

ALTER TABLE class_schedules
    ADD COLUMN IF NOT EXISTS room_id BIGINT REFERENCES rfid_devices(id) ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_class_schedules_room ON class_schedules (room_id) WHERE room_id IS NOT NULL;
