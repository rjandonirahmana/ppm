-- =============================================================================
-- 48_unstick_pamongless_classes.sql — Lepaskan izin & absensi yang MACET
-- permanen karena kelasnya menuntut pamong tapi belum punya pamong.
--
-- MASALAH:
--   Kelas dengan `require_pamong = TRUE` tapi `pamong_id IS NULL` menciptakan
--   jalan buntu. Query antrean pamong menyaring dengan
--   `COALESCE(pamong_id, ...) = $pamong` → NULL = id → tak pernah cocok, jadi
--   TIDAK ADA pamong yang melihatnya. Sementara tahap final menuntut
--   `pamong_status = 'approved'`, jadi wali kelas pun terblokir. Admin bisa
--   melihat tapi tak punya jalur kode untuk menyetujui tahap pamong
--   (service/permits.rs & service/attendance.rs hanya merutekan peran
--   `supervisor` ke tahap itu, selalu dengan id dirinya).
--
--   Akibatnya: izin menggantung selamanya, dan absensi tak pernah final →
--   SANTRI TIDAK PERNAH MENERIMA POIN KEHADIRANNYA.
--
-- PERBAIKAN KODE (sudah menyertai migrasi ini):
--   Tahap pamong kini hanya berlaku bila pamongnya BENAR-BENAR ADA. Kelas
--   ber-require_pamong tanpa pamong → tahap pamong dilewati, langsung ke wali
--   kelas. Jadi kejadian ini tak akan terulang.
--
-- Migrasi ini menangani data yang TERLANJUR macet + memberi alat deteksi.
-- Idempotent. Jalankan setelah migrasi 1–47.
-- =============================================================================

-- ═ 1) Lihat dulu dampaknya — JALANKAN INI SEBELUM bagian (2) ════════════════
-- Kelas bermasalah:
--   SELECT id, name, require_pamong, pamong_id, wali_kelas_id
--     FROM classes WHERE require_pamong AND pamong_id IS NULL;
--
-- Izin yang macet karenanya:
--   SELECT p.id, u.full_name, p.start_date, p.end_date, c.name AS kelas
--     FROM permit_requests p
--     JOIN users u ON u.id = p.user_id
--     JOIN classes c ON c.id = p.class_id
--    WHERE p.pamong_status = 'pending' AND p.guru_status = 'pending'
--      AND c.require_pamong AND c.pamong_id IS NULL;
--
-- Absensi yang macet (ini yang menahan poin santri):
--   SELECT COUNT(*) FROM attendances a
--     JOIN class_sessions cs ON cs.id = a.class_session_id
--     JOIN classes cl ON cl.id = cs.class_id
--    WHERE a.verify_status = 'pending' AND a.pamong_status = 'pending'
--      AND cl.require_pamong AND COALESCE(cs.pamong_id, cl.pamong_id) IS NULL;

-- ═ 2) Tandai tahap pamong sebagai DILEWATI, bukan disetujui ═════════════════
-- Memakai 'approved' akan berbohong: seolah ada pamong yang memeriksa. Tapi
-- CHECK constraint kolom ini hanya mengizinkan pending/approved/rejected, jadi
-- 'approved' dipakai dengan CATATAN di kolom note/reason agar jejaknya jujur.
--
-- Izin: lepaskan ke antrean wali kelas.
UPDATE permit_requests p
   SET pamong_status = 'approved',
       pamong_at = COALESCE(p.pamong_at, NOW()),
       pamong_note = COALESCE(p.pamong_note, '')
                     || '[sistem] Tahap pamong dilewati: kelas belum punya pamong (migrasi 48).'
  FROM classes c
 WHERE c.id = p.class_id
   AND p.pamong_status = 'pending'
   AND p.guru_status = 'pending'
   AND c.require_pamong
   AND c.pamong_id IS NULL;

-- Absensi: lepaskan ke verifikasi final. Poin TIDAK diberikan di sini — tetap
-- lewat jalur normal (decide_verify / run_auto_verify_final) supaya aturan poin
-- hanya hidup di satu tempat.
UPDATE attendances a
   SET pamong_status = 'approved',
       pamong_at = COALESCE(a.pamong_at, NOW()),
       note = COALESCE(a.note || ' ', '')
              || '[sistem] Tahap pamong dilewati: kelas belum punya pamong (migrasi 48).'
  FROM class_sessions cs
  JOIN classes cl ON cl.id = cs.class_id
 WHERE cs.id = a.class_session_id
   AND a.verify_status = 'pending'
   AND a.pamong_status = 'pending'
   AND cl.require_pamong
   AND COALESCE(cs.pamong_id, cl.pamong_id) IS NULL;

-- ═ 3) Deteksi dini, bukan larangan ══════════════════════════════════════════
-- SENGAJA TIDAK memakai CHECK (require_pamong = FALSE OR pamong_id IS NOT NULL):
-- admin wajar membuat kelas dulu lalu menunjuk pamong belakangan, dan constraint
-- itu akan menolak alur kerja yang sah. Kode kini sudah tahan terhadap keadaan
-- ini; yang dibutuhkan cuma cara melihatnya.
CREATE OR REPLACE VIEW v_classes_missing_pamong AS
    SELECT id, name, wali_kelas_id
      FROM classes
     WHERE require_pamong AND pamong_id IS NULL;

COMMENT ON VIEW v_classes_missing_pamong IS
    'Kelas yang menuntut verifikasi pamong tapi belum ditunjuk pamongnya. Izin & absensi kelas ini melewati tahap pamong (lihat migrasi 48). Tunjuk pamong bila memang perlu 2 tahap.';
