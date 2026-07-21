-- =============================================================================
-- 12_gate_pondok.sql — RFID gerbang UTAMA pondok (masuk/keluar), TERPISAH dari
-- gerbang kelas (rfid_devices + POST /api/rfid/scan sudah ada). Device gerbang
-- pondok = baris rfid_devices BARU (device_name mis. "Gerbang Utama"), tapi
-- di-arahkan ke endpoint BEDA: POST /api/rfid/gate — jadi TIDAK perlu kolom/
-- tabel device baru, cukup device_name/location beda + firmware pukul URL beda.
--
-- Toggle otomatis: satu tap = balik status dari terakhir (in→out, out→in) —
-- firmware tak perlu tahu arah, cukup kirim api_key+card sama seperti gerbang
-- kelas. users.gate_status = CACHE status terkini (pola sama users.points),
-- gate_logs = riwayat lengkap (audit + laporan "kapan keluar/masuk").
--
-- Idempotent. Jalankan setelah migrasi 1–11.
-- =============================================================================

ALTER TABLE users ADD COLUMN IF NOT EXISTS gate_status VARCHAR(3) NOT NULL DEFAULT 'in'
    CHECK (gate_status IN ('in', 'out'));
ALTER TABLE users ADD COLUMN IF NOT EXISTS gate_at TIMESTAMPTZ;

CREATE TABLE IF NOT EXISTS gate_logs (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id   BIGINT REFERENCES rfid_devices(id) ON DELETE SET NULL,
    direction   VARCHAR(3) NOT NULL CHECK (direction IN ('in', 'out')),
    scanned_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_gate_logs_user ON gate_logs (user_id, scanned_at DESC);
CREATE INDEX IF NOT EXISTS idx_users_gate_out ON users (gate_status) WHERE gate_status = 'out';
