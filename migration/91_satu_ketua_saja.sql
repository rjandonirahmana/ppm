-- =============================================================================
-- 91_satu_ketua_saja.sql — Ketua itu JABATAN: hanya boleh ada satu, dan
-- basis data yang menjaminnya.
--
-- ── KENAPA DI SINI, BUKAN CUKUP DI KODE ──────────────────────────────────────
-- `service::admin::change_role` sudah memperlakukan penunjukan ketua sebagai
-- PERPINDAHAN — yang ditunjuk naik, yang lama turun jadi admin, satu transaksi.
-- Itu menutup jalur aplikasinya. Yang TIDAK ditutupnya: `UPDATE users SET
-- role='ketua'` yang diketik langsung ke produksi, impor data, atau endpoint
-- baru yang kelak lupa memakai jalur perpindahan itu.
--
-- Invarian sepenting ini — ketua adalah satu-satunya peran yang memegang
-- keuangan DAN boleh menghapus akun beserta seluruh riwayatnya — tak pantas
-- bersandar pada ingatan penulis kode berikutnya. Index di bawah membuat
-- pelanggarannya MUSTAHIL, bukan sekadar tak dianjurkan.
--
-- ── BAGAIMANA CARANYA ────────────────────────────────────────────────────────
-- Index unik PARSIAL atas ekspresi TETAP: seluruh baris ber-`role = 'ketua'`
-- memetakan ke kunci yang sama (angka 1), jadi baris kedua langsung ditolak.
-- Baris dengan peran lain tak ikut ter-index sama sekali — tak ada biaya tulis
-- untuk 500-an santri yang tak ada urusannya dengan ini.
--
-- Konsekuensi yang HARUS dipahami penulis kode berikutnya: mengangkat ketua
-- baru WAJIB menurunkan yang lama LEBIH DULU dalam transaksi yang sama.
-- Urutan terbalik menabrak index ini. `repository::transfer_ketua` sudah
-- melakukannya dengan urutan yang benar.
--
-- Idempoten. Aman diulang.
-- =============================================================================

-- ═ 1) Rapikan keadaan sekarang: sisakan SATU ketua ═══════════════════════════
--
-- Yang dipertahankan adalah id TERKECIL — akun ketua yang paling lama, yang
-- hampir pasti pemegang jabatan sesungguhnya; yang lahir belakangan biasanya
-- hasil salah setel. Pilihan ini bisa saja keliru pada suatu database, karena
-- itu ia DILAPORKAN, bukan dikerjakan diam-diam: bila yang benar justru yang
-- lain, tinggal serahkan jabatannya lewat /manajemen-user sesudah ini.
DO $$
DECLARE
    n_ketua int;
    r       record;
BEGIN
    SELECT count(*) INTO n_ketua FROM users WHERE role = 'ketua';
    RAISE NOTICE 'Akun ber-peran ketua saat ini: %', n_ketua;

    IF n_ketua > 1 THEN
        FOR r IN
            SELECT id, full_name FROM users
             WHERE role = 'ketua'
               AND id > (SELECT min(id) FROM users WHERE role = 'ketua')
             ORDER BY id
        LOOP
            RAISE NOTICE '  diturunkan jadi admin: id=% "%"', r.id, r.full_name;
        END LOOP;

        UPDATE users
           SET role = 'admin', updated_at = NOW()
         WHERE role = 'ketua'
           AND id > (SELECT min(id) FROM users WHERE role = 'ketua');
    END IF;

    -- NOL ketua bukan galat, dan tak diperbaiki di sini. Instalasi baru memang
    -- mulai tanpa ketua; `service::admin::change_role` memberi jalan keluarnya
    -- (admin boleh menunjuk ketua PERTAMA selagi jabatannya masih kosong).
    IF n_ketua = 0 THEN
        RAISE NOTICE 'Belum ada ketua. Tunjuk lewat /manajemen-user — selagi kosong, admin boleh menunjuk yang pertama.';
    END IF;
END $$;

-- ═ 2) Kunci invariannya ══════════════════════════════════════════════════════
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_satu_ketua
    ON users ((1))
 WHERE role = 'ketua';

COMMENT ON INDEX uq_users_satu_ketua IS
    'Ketua hanya boleh satu. Mengangkat ketua baru wajib menurunkan yang lama '
    'lebih dulu dalam transaksi yang sama — lihat repository::transfer_ketua.';

-- =============================================================================
-- VERIFIKASI:
--   SELECT id, full_name, role FROM users WHERE role = 'ketua';   -- 0 atau 1 baris
--
--   -- Index-nya benar-benar ada dan parsial:
--   SELECT indexname, indexdef FROM pg_indexes
--    WHERE tablename = 'users' AND indexname = 'uq_users_satu_ketua';
--
--   -- Membuktikan penjaganya hidup (HARUS gagal bila sudah ada ketua):
--   --   UPDATE users SET role='ketua' WHERE id = <id lain>;
--   --   → ERROR: duplicate key value violates unique constraint "uq_users_satu_ketua"
-- =============================================================================
