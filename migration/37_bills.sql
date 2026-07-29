-- 37_bills.sql — Tagihan santri (SPP dll). Dilihat/kelola oleh admin, ketua,
-- dan santri_finance (santri pemegang kunci finance). Santri lihat tagihannya
-- sendiri + unggah bukti bayar.

CREATE TABLE IF NOT EXISTS bills (
    id           BIGSERIAL PRIMARY KEY,
    user_id      BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    title        TEXT        NOT NULL DEFAULT '',   -- "SPP Juli 2026"
    price        BIGINT      NOT NULL,              -- nominal tagihan (rupiah)
    started_date DATE        NOT NULL,
    expired_date DATE        NOT NULL,
    status       TEXT        NOT NULL DEFAULT 'belum', -- belum | lunas
    paid_at      TIMESTAMPTZ,
    paid_amount  BIGINT,                            -- nominal dibayar (cicilan/kurang)
    method       TEXT,                              -- transfer | tunai
    proof_url    TEXT,                              -- bukti bayar (foto) di RustFS
    verified_by  BIGINT      REFERENCES users(id) ON DELETE SET NULL,
    note         TEXT        NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_bills_user   ON bills (user_id);
CREATE INDEX IF NOT EXISTS idx_bills_status ON bills (status, expired_date);
