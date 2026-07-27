-- =============================================================================
-- 32_points_trigger.sql — JAMIN users.points selalu = akumulasi point_logs.
--
-- MASALAH: pemberian/pengurangan poin tersebar di banyak jalur (decide_pamong,
-- run_auto_absent, run_auto_verify, reward mingguan, dll). Tiap jalur menulis
-- point_logs LALU meng-UPDATE users.points manual. Kalau satu jalur bug (mis.
-- CTE visibility di run_auto_absent), point_logs & users.points bisa MELESET.
--
-- SOLUSI: trigger tunggal di point_logs → SETIAP insert/update/delete otomatis
-- menyesuaikan users.points. Jadi cukup tulis point_logs; saldo pasti ikut.
-- (Kode aplikasi tak lagi meng-UPDATE users.points manual → tak dobel hitung.)
--
-- Idempotent. Setelah migrasi 1–31.
-- =============================================================================

CREATE OR REPLACE FUNCTION apply_point_log() RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE users SET points = points + NEW.delta WHERE id = NEW.user_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE users SET points = points - OLD.delta WHERE id = OLD.user_id;
    ELSIF TG_OP = 'UPDATE' THEN
        IF NEW.user_id = OLD.user_id THEN
            UPDATE users SET points = points + NEW.delta - OLD.delta WHERE id = NEW.user_id;
        ELSE
            UPDATE users SET points = points - OLD.delta WHERE id = OLD.user_id;
            UPDATE users SET points = points + NEW.delta WHERE id = NEW.user_id;
        END IF;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_point_logs_balance ON point_logs;
CREATE TRIGGER trg_point_logs_balance
    AFTER INSERT OR UPDATE OR DELETE ON point_logs
    FOR EACH ROW EXECUTE FUNCTION apply_point_log();

-- ── Backfill: point_log absent yang TERLEWAT (bug CTE run_auto_absent) ────────
-- Deteksi per santri: jumlah 'absent' attendance > jumlah point_log absent yang
-- ada → sisipkan kekurangannya (magnitudo dari jadwal). Trigger di atas otomatis
-- mengoreksi users.points. Idempotent (LIKE mencakup log rekonsiliasi → re-run
-- tak menambah lagi).
WITH ranked_absent AS (
    SELECT a.user_id, a.class_schedule_id,
           row_number() OVER (PARTITION BY a.user_id ORDER BY a.scanned_at) AS rn
    FROM attendances a
    WHERE a.status = 'absent'
),
logcount AS (
    SELECT user_id, COUNT(*) AS c
    FROM point_logs
    WHERE reason LIKE 'Kehadiran (absent)%'
    GROUP BY user_id
)
INSERT INTO point_logs (user_id, delta, reason, category)
SELECT ra.user_id,
       -COALESCE(sch.absent_points, cat_default_points(COALESCE(sch.activity_type, 'other'), 'absent'))::int,
       'Kehadiran (absent) — rekonsiliasi', 'discipline'
FROM ranked_absent ra
LEFT JOIN class_schedules sch ON sch.id = ra.class_schedule_id
LEFT JOIN logcount lc ON lc.user_id = ra.user_id
WHERE ra.rn > COALESCE(lc.c, 0);
