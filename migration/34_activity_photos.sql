-- 34_activity_photos.sql — Galeri "Foto Kegiatan" untuk beranda publik.
-- Diunggah staf (admin/dewan_guru) ke RustFS; ditampilkan terurut `sort_order`
-- (bisa di-drag di halaman /galeri). caption opsional.

CREATE TABLE IF NOT EXISTS activity_photos (
    id         BIGSERIAL PRIMARY KEY,
    url        TEXT        NOT NULL,
    caption    TEXT        NOT NULL DEFAULT '',
    sort_order INT         NOT NULL DEFAULT 0,
    created_by BIGINT      REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Urutan tampil galeri (sort_order lalu id sebagai tie-breaker stabil).
CREATE INDEX IF NOT EXISTS idx_activity_photos_sort
    ON activity_photos (sort_order, id);
