-- 35_guest_visits.sql — Buku tamu. Tamu isi form di /tamu → dapat kode 6-digit
-- (disimpan di Redis, bukan tabel ini) → ketik di mesin IoT → mesin ambil wajah
-- & kirim kode → baris ini dibuat SAAT check-in berhasil (wajah tersimpan).

CREATE TABLE IF NOT EXISTS guest_visits (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT        NOT NULL,
    phone         TEXT        NOT NULL,
    purpose       TEXT        NOT NULL DEFAULT '',
    face_url      TEXT,                       -- foto wajah di RustFS (bukti hadir)
    device_id     BIGINT      REFERENCES rfid_devices(id) ON DELETE SET NULL,
    checked_in_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_guest_visits_time ON guest_visits (checked_in_at DESC);
