-- =============================================================================
-- 28_prd_points.sql — Fondasi poin sesuai PRD "Sistem Poin 2.0" PPM AFM.
--
-- 1) Saldo awal santri = 300 (PRD: diberi 300 tiap awal semester, berkurang bila
--    izin/alfa/telat, direset tiap semester). Default kolom → 300 untuk user baru.
-- 2) Jenis kegiatan per-jadwal (activity_type): kbm | non_kbm | piket |
--    apel_kepulangan → menentukan PRESET poin default (bisa di-override per
--    jadwal lewat present/late/absent/izin_points).
-- 3) izin_points: poin dikurangi saat IZIN (biasa) — PRD: KBM −3, Non-KBM −2,
--    Apel Kepulangan −5. Sakit/Cuti TIDAK mengurangi (ditangani di kode).
-- 4) Fungsi cat_default_points() = SATU sumber kebenaran preset PRD, dipakai
--    operasi bulk SQL (run_auto_absent / run_auto_verify_pamong). Rust
--    models::category_points() MENCERMINKAN nilai ini — ubah keduanya bersama.
--
-- Preset PRD (magnitudo positif; arah operasi di kode):
--   kind\type       kbm  non_kbm  piket  apel_kepulangan  (lainnya/legacy)
--   present(+)       4      3       1          0                10
--   late/telat(−)    1      1       0          0                 0
--   absent/alfa(−)  10      5       2         20                15
--   izin(−)          3      2       0          5                 0
-- (Non-KBM alfa PRD "−5 s.d −10 tergantung kegiatan" → default 5, override bila perlu.)
--
-- Idempotent. Setelah migrasi 1–27.
-- =============================================================================

ALTER TABLE users ALTER COLUMN points SET DEFAULT 300;

ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS activity_type VARCHAR(20);
ALTER TABLE class_schedules ADD COLUMN IF NOT EXISTS izin_points   SMALLINT;

CREATE OR REPLACE FUNCTION cat_default_points(atype text, kind text) RETURNS int AS $$
  SELECT CASE kind
    WHEN 'present' THEN CASE atype WHEN 'kbm' THEN 4  WHEN 'non_kbm' THEN 3 WHEN 'piket' THEN 1 WHEN 'apel_kepulangan' THEN 0  ELSE 10 END
    WHEN 'late'    THEN CASE atype WHEN 'kbm' THEN 1  WHEN 'non_kbm' THEN 1 WHEN 'piket' THEN 0 WHEN 'apel_kepulangan' THEN 0  ELSE 0  END
    WHEN 'absent'  THEN CASE atype WHEN 'kbm' THEN 10 WHEN 'non_kbm' THEN 5 WHEN 'piket' THEN 2 WHEN 'apel_kepulangan' THEN 20 ELSE 15 END
    WHEN 'izin'    THEN CASE atype WHEN 'kbm' THEN 3  WHEN 'non_kbm' THEN 2 WHEN 'piket' THEN 0 WHEN 'apel_kepulangan' THEN 5  ELSE 0  END
  END
$$ LANGUAGE sql IMMUTABLE;
