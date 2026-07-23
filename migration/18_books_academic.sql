-- =============================================================================
-- 18_books_academic.sql — Buku materi hafalan (Qur'an/Hadist) + progres per
-- santri per buku.
--
-- BEDA dari hafalan_logs (migrasi 11): hafalan_logs = LOG append-only tiap
-- setoran (surah/ayat/juz per sesi). Di sini `books` = daftar REFERENSI buku
-- (mis. "Sahih Bukhari", "Al-Qur'an") dgn total halaman, dan `academic_user`
-- = STATUS TERKINI satu santri pada satu buku (persentase + halaman yang
-- belum/kosong) — satu baris per (santri, buku), ditimpa (upsert) tiap
-- diperbarui admin/pamong, BUKAN riwayat.
--
-- missing_pages: JSONB array pasangan [awal, akhir], mis. [[11,20],[45,50]].
-- Dipilih atas TEKS bebas krn perlu di-query (jsonb_array_length utk "santri
-- mana yang masih ada bolong", dst) tanpa parsing string di aplikasi — form
-- input tetap SATU kotak teks ("11-20, 45-50"), diparse/diformat ke/dari
-- JSONB ini di service layer (bukan tipe range native Postgres: tak ada
-- dukungan tokio-postgres tanpa crate tambahan, tak sepadan utk kebutuhan
-- sekarang yang cuma tampilan + statistik sederhana).
--
-- Idempotent. Jalankan setelah migrasi 1–17.
-- =============================================================================

CREATE TABLE IF NOT EXISTS books (
    id           BIGSERIAL PRIMARY KEY,
    title        VARCHAR(200) NOT NULL,
    total_pages  INTEGER NOT NULL CHECK (total_pages > 0),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    deleted_at   TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_books_active ON books (title) WHERE deleted_at IS NULL;

CREATE TABLE IF NOT EXISTS academic_user (
    id             BIGSERIAL PRIMARY KEY,
    user_id        BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    book_id        BIGINT NOT NULL REFERENCES books(id) ON DELETE CASCADE,
    percentage     SMALLINT NOT NULL DEFAULT 0 CHECK (percentage BETWEEN 0 AND 100),
    missing_pages  JSONB NOT NULL DEFAULT '[]'::jsonb,
    updated_by     BIGINT REFERENCES users(id) ON DELETE SET NULL,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at     TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (user_id, book_id)
);
CREATE INDEX IF NOT EXISTS idx_academic_user_user ON academic_user (user_id);
CREATE INDEX IF NOT EXISTS idx_academic_user_book ON academic_user (book_id);
