-- =============================================================================
-- 62_class_verify_mode.sql — Mode verifikasi absensi jadi TIGA pilihan per kelas.
--
-- Sebelumnya cuma `classes.require_pamong` (boolean) = dua kemungkinan:
--   TRUE  → pamong dulu, lalu dewan guru (dua langkah)
--   FALSE → langsung dewan guru (satu langkah)
-- Kenyataannya ada kelas yang cukup diverifikasi PAMONG saja — dan itu tak bisa
-- diungkapkan boolean tersebut sama sekali.
--
-- `verify_mode`:
--   'dua_tahap' → pamong menyetujui, lalu dewan guru memfinalkan (poin di final)
--   'guru'      → cukup dewan guru/guru; tahap pamong dilewati
--   'pamong'    → cukup pamong; pamonglah yang memfinalkan (poin di situ)
--
-- `require_pamong` DIPERTAHANKAN sementara: enam query masih membacanya, dan
-- mengganti semuanya sekaligus di migrasi yang sama membuat perubahan ini sulit
-- dibalik bila keliru. Nilainya dijaga tetap SEPADAN lewat trigger di bawah,
-- jadi kode lama dan baru tak bisa berbeda pendapat selama masa peralihan.
--
-- Idempotent. Jalankan setelah migrasi 1–61.
-- =============================================================================

ALTER TABLE classes
    ADD COLUMN IF NOT EXISTS verify_mode VARCHAR(20);

-- Turunkan dari keadaan sekarang supaya perilaku kelas yang sudah ada TIDAK
-- berubah sedikit pun saat migrasi dijalankan.
UPDATE classes
   SET verify_mode = CASE WHEN COALESCE(require_pamong, TRUE) THEN 'dua_tahap' ELSE 'guru' END
 WHERE verify_mode IS NULL;

ALTER TABLE classes ALTER COLUMN verify_mode SET DEFAULT 'dua_tahap';
ALTER TABLE classes ALTER COLUMN verify_mode SET NOT NULL;

ALTER TABLE classes DROP CONSTRAINT IF EXISTS chk_classes_verify_mode;
ALTER TABLE classes ADD CONSTRAINT chk_classes_verify_mode
    CHECK (verify_mode IN ('dua_tahap', 'guru', 'pamong'));

-- Jaga `require_pamong` tetap sepadan selama masa peralihan: hanya mode
-- 'dua_tahap' yang membutuhkan tahap pamong. Tanpa ini, query lama yang masih
-- membaca require_pamong bisa mengambil keputusan berbeda dari yang baru.
CREATE OR REPLACE FUNCTION sync_require_pamong() RETURNS trigger AS $$
BEGIN
    NEW.require_pamong := (NEW.verify_mode = 'dua_tahap');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_sync_require_pamong ON classes;
CREATE TRIGGER trg_sync_require_pamong
    BEFORE INSERT OR UPDATE OF verify_mode ON classes
    FOR EACH ROW EXECUTE FUNCTION sync_require_pamong();

UPDATE classes SET require_pamong = (verify_mode = 'dua_tahap');

-- Verifikasi:
--   SELECT id, name, verify_mode, require_pamong FROM classes ORDER BY id;
