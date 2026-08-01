-- =============================================================================
-- 46_permit_per_class.sql — Izin PER-KELAS + hapus persetujuan orang tua.
--
-- MASALAH yang diperbaiki:
--   Sebelumnya SATU izin = SATU baris, disetujui orang tua → pamong kelas UTAMA
--   → wali kelas UTAMA. Padahal santri yang izin 2–3 hari bisa melewati kelas
--   dengan WALI KELAS BERBEDA — wali kelas B tak pernah tahu santrinya absen
--   karena yang dimintai persetujuan hanya wali kelas utama.
--
-- ALUR BARU:
--   Satu pengajuan DIPECAH jadi beberapa baris `permit_requests` — satu untuk
--   tiap wali kelas unik yang kelasnya dilewati selama rentang izin. Contoh:
--   izin 2 hari melewati kelas A (wali X), B & C (wali Y) → 2 baris:
--     • baris 1 → wali X (kelas A)
--     • baris 2 → wali Y (kelas B & C — penyetuju sama, cukup sekali)
--   Tiap baris: pamong kelas (bila require_pamong) → wali kelas (FINAL).
--
--   Orang tua BUKAN penyetuju lagi (izin = urusan akademik). Mereka tetap bisa
--   MENGAJUKAN izin untuk anaknya dan melihat statusnya.
--
-- Idempotent. Jalankan setelah migrasi 1–45.
-- =============================================================================

-- ═ 1) Kolom baru: kelas tujuan izin + wali kelas penyetujunya ════════════════
-- class_id     = kelas acuan approval (menentukan require_pamong & pamong).
-- wali_kelas_id= disimpan eksplisit agar antrean wali kelas tetap stabil
--                walau wali kelas diganti setelah izin diajukan.
ALTER TABLE permit_requests
    ADD COLUMN IF NOT EXISTS class_id BIGINT REFERENCES classes(id) ON DELETE SET NULL;
ALTER TABLE permit_requests
    ADD COLUMN IF NOT EXISTS wali_kelas_id BIGINT REFERENCES users(id) ON DELETE SET NULL;

-- ═ 2) Backfill izin LAMA dari kelas utama santri ═════════════════════════════
-- Dijalankan SEBELUM kolom parent_* dihapus supaya bisa di-rollback manual bila
-- perlu (lihat catatan rollback di bawah).
UPDATE permit_requests pr
   SET class_id = sub.class_id,
       wali_kelas_id = sub.wali_kelas_id
  FROM (
        SELECT cp.user_id, cp.class_id, c.wali_kelas_id
          FROM class_participants cp
          JOIN classes c ON c.id = cp.class_id
         WHERE cp.is_primary
       ) AS sub
 WHERE pr.user_id = sub.user_id
   AND pr.class_id IS NULL;

-- ═ 3) Izin yang masih tertahan di tahap orang tua → lepaskan ═════════════════
-- Baris dengan parent_status='pending' akan MACET selamanya setelah kolomnya
-- dihapus (query approval lama menuntut parent_status='approved'). Lepaskan ke
-- antrean akademik. Baris yang sudah DITOLAK orang tua ditandai ditolak wali
-- kelas agar keputusan historisnya tak hilang.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
         WHERE table_name = 'permit_requests' AND column_name = 'parent_status'
    ) THEN
        UPDATE permit_requests
           SET guru_status = 'rejected',
               guru_at = COALESCE(guru_at, NOW())
         WHERE parent_status = 'rejected' AND guru_status = 'pending';
    END IF;
END $$;

-- ═ 4) Hapus kolom persetujuan orang tua ══════════════════════════════════════
-- ROLLBACK: kolom ini tak bisa dikembalikan isinya. Ambil dump tabel
-- permit_requests sebelum menjalankan migrasi ini di produksi.
ALTER TABLE permit_requests DROP COLUMN IF EXISTS parent_status;
ALTER TABLE permit_requests DROP COLUMN IF EXISTS parent_confirmed_by;
ALTER TABLE permit_requests DROP COLUMN IF EXISTS parent_confirmed_at;

-- ═ 5) Index antrean per penyetuju ════════════════════════════════════════════
-- Antrean wali kelas: WHERE wali_kelas_id = $1 AND guru_status = 'pending'.
CREATE INDEX IF NOT EXISTS idx_permit_guru_pending
    ON permit_requests (wali_kelas_id, guru_status, created_at)
    WHERE guru_status = 'pending';

-- Antrean pamong: WHERE class_id = ... AND pamong_status = 'pending'.
CREATE INDEX IF NOT EXISTS idx_permit_pamong_pending
    ON permit_requests (class_id, pamong_status, created_at)
    WHERE pamong_status = 'pending';

-- ═ 6) Cegah duplikat ajuan untuk kelas & rentang yang sama ═══════════════════
-- CATATAN: TIDAK memakai COALESCE(end_date, start_date) di dalam index —
-- ekspresi itu IMMUTABLE sebenarnya, tapi index parsial atas kolom nullable
-- lebih sederhana & aman. Baris yang sudah ditolak dikecualikan supaya santri
-- boleh mengajukan ulang setelah ditolak.
CREATE UNIQUE INDEX IF NOT EXISTS uq_permit_class_range
    ON permit_requests (user_id, class_id, start_date)
    WHERE guru_status <> 'rejected' AND pamong_status <> 'rejected'
      AND class_id IS NOT NULL;

-- ═ 7) Verifikasi (jalankan MANUAL di staging sebelum produksi) ═══════════════
-- Izin tanpa kelas acuan (santri tanpa kelas utama) — approval jatuh ke
-- fallback kelas utama di kode; harusnya 0 setelah backfill:
--   SELECT COUNT(*) FROM permit_requests WHERE class_id IS NULL;
--
-- Izin menggantung tanpa penyetuju (kelas tanpa wali kelas) — perlu dewan
-- guru/admin yang memutuskan:
--   SELECT COUNT(*) FROM permit_requests
--    WHERE wali_kelas_id IS NULL AND guru_status = 'pending';
