-- =============================================================================
-- 75_pengajuan_pembayaran.sql — Pembayaran DIAJUKAN santri/orang tua.
--
-- ARAH ALURNYA BERBALIK. Sampai sekarang `bills` selalu lahir dari pengurus:
-- ia menetapkan nominal dan periode lebih dulu, santri tinggal membayar. Yang
-- sebenarnya terjadi di pondok ini kebalikannya — keluarga mentransfer sejumlah
-- uang, lalu pengurus menentukan periode mana yang tertutup oleh setoran itu.
-- Alur lama TIDAK dibuang (pengurus tetap bisa mencatat langsung); yang
-- ditambahkan di sini adalah jalur pengajuan.
--
-- Akibatnya pada skema:
--   • `started_date`/`expired_date` harus BOLEH KOSONG. Saat santri mengirim
--     bukti transfer, periodenya memang belum ada — itu justru yang diputuskan
--     verifikator. Memaksa NOT NULL berarti mengarang tanggal lalu menimpanya,
--     dan tanggal karangan itu terlanjur terbaca sebagai fakta oleh layar mana
--     pun yang membacanya di antara dua langkah tersebut.
--   • `status` bertambah 'menunggu' (diajukan, belum diperiksa) dan 'ditolak'
--     (bukti tak cocok dengan mutasi rekening).
--
-- `price` TETAP NOT NULL: saat pengajuan ia diisi nominal yang DIAKUI penyetor,
-- dan saat verifikasi `paid_amount` diisi nominal yang BENAR-BENAR masuk.
-- Keduanya sengaja dibiarkan bisa berbeda — selisihnya itulah yang perlu
-- terlihat, bukan disamarkan jadi satu angka.
--
-- Idempotent. Jalankan setelah migrasi 1–74.
-- =============================================================================

-- ── 1. Periode baru diisi saat verifikasi ────────────────────────────────────
ALTER TABLE bills ALTER COLUMN started_date DROP NOT NULL;
ALTER TABLE bills ALTER COLUMN expired_date DROP NOT NULL;

-- ── 2. Jejak pengajuan ───────────────────────────────────────────────────────
-- `submitted_by` DIPISAH dari `user_id`: orang tua boleh mengajukan atas nama
-- anaknya, dan saat ada sengketa ("siapa yang mengunggah bukti ini?") kolom
-- inilah yang menjawab. ON DELETE SET NULL — pengaju bisa saja sudah lulus,
-- catatan keuangannya tidak boleh ikut hilang (semangat yang sama dgn migrasi 70).
ALTER TABLE bills ADD COLUMN IF NOT EXISTS submitted_by BIGINT
    REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE bills ADD COLUMN IF NOT EXISTS submitted_at TIMESTAMPTZ;

-- Alasan penolakan. Kolom sendiri, bukan menumpang `note`: yang ini DIBACA
-- SANTRI di layarnya, sedangkan `note` adalah catatan internal pengurus —
-- menggabungkannya berarti catatan internal ikut terbaca keluarga.
ALTER TABLE bills ADD COLUMN IF NOT EXISTS reject_reason TEXT;

-- ── 3. Status yang sah ───────────────────────────────────────────────────────
-- CHECK-nya baru ada sekarang (migrasi 37 hanya menuliskannya sebagai komentar).
-- Tanpa ini satu salah ketik 'Belum' membuat tagihan lenyap dari daftar
-- belum-lunas DAN dari deteksi keterlambatan sekaligus, tanpa galat apa pun.
ALTER TABLE bills DROP CONSTRAINT IF EXISTS chk_bills_status;
ALTER TABLE bills ADD CONSTRAINT chk_bills_status
    CHECK (status IN ('belum', 'menunggu', 'lunas', 'ditolak'));

ALTER TABLE bills DROP CONSTRAINT IF EXISTS chk_bills_amount;
ALTER TABLE bills ADD CONSTRAINT chk_bills_amount
    CHECK (price >= 0 AND (paid_amount IS NULL OR paid_amount >= 0));

-- Hanya berlaku bila KEDUANYA terisi — baris pengajuan yang belum diverifikasi
-- memang belum punya periode.
ALTER TABLE bills DROP CONSTRAINT IF EXISTS chk_bills_dates;
ALTER TABLE bills ADD CONSTRAINT chk_bills_dates
    CHECK (started_date IS NULL OR expired_date IS NULL OR expired_date >= started_date);

-- ── 4. Jejak pengingat WhatsApp ──────────────────────────────────────────────
-- Di `users`, bukan di `bills`: yang diingatkan adalah ORANGNYA ("masa
-- berlakumu habis"), dan sering justru karena ia belum punya baris bills sama
-- sekali. Dipakai layar untuk menampilkan "sudah diingatkan 2 hari lalu"
-- supaya keluarga yang sama tak ditagih berkali-kali oleh pengurus berbeda.
ALTER TABLE users ADD COLUMN IF NOT EXISTS bill_reminded_at TIMESTAMPTZ;

-- ── 5. Index ─────────────────────────────────────────────────────────────────
-- Nama-nama BARU, tak menyerupai index mana pun yang sudah ada: `CREATE INDEX
-- IF NOT EXISTS` mencocokkan NAMA, bukan definisi, jadi memakai ulang nama lama
-- membuat migrasi ini terlihat sukses padahal indexnya tak pernah terbuat
-- (jebakan yang sudah menggigit dua kali — lihat migrasi 45 & 70).

-- Antrean verifikasi: kecil, sering dibaca, terurut pengajuan terlama dulu.
CREATE INDEX IF NOT EXISTS idx_bills_antrean_verifikasi
    ON bills (submitted_at) WHERE status = 'menunggu';

-- "Periode terlewat" mencari expired_date TERBESAR per santri di antara baris
-- lunas. Index parsial ini yang membuat pencarian itu tak memindai seluruh
-- riwayat pembayaran pondok tiap kali halamannya dibuka.
CREATE INDEX IF NOT EXISTS idx_bills_lunas_per_santri
    ON bills (user_id, expired_date DESC) WHERE status = 'lunas';

ANALYZE bills;
ANALYZE users;

-- =============================================================================
-- VERIFIKASI MANUAL (jalankan sesudahnya; semuanya harus mengembalikan baris):
--
--   SELECT column_name, is_nullable FROM information_schema.columns
--    WHERE table_name = 'bills'
--      AND column_name IN ('started_date','expired_date','submitted_by',
--                          'submitted_at','reject_reason');
--   -- started_date & expired_date HARUS 'YES'; tiga sisanya harus ADA.
--
--   SELECT conname FROM pg_constraint
--    WHERE conrelid = 'bills'::regclass AND conname LIKE 'chk_bills%';
--   -- harus 3 baris.
--
--   -- Bandingkan DEFINISI, bukan sekadar nama (lihat catatan di bagian 5):
--   SELECT indexrelid::regclass, pg_get_indexdef(indexrelid)
--     FROM pg_index WHERE indrelid = 'bills'::regclass;
-- =============================================================================
