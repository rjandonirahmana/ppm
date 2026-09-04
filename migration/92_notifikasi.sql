-- 92_notifikasi.sql — Notifikasi DI DALAM aplikasi.
--
-- ── KENAPA ADA ──────────────────────────────────────────────────────────────
-- Sampai sekarang satu-satunya pemberitahuan izin adalah pesan WhatsApp ke wali
-- kelas (`service::permits::notify_permit_splits`). Dua hal yang tak
-- tertangani olehnya:
--
--   1. Pesan WA hanya dikirim ke WALI KELAS. Admin tak pernah tahu ada izin
--      masuk kecuali membuka layarnya sendiri.
--   2. SANTRI tak pernah diberi tahu keputusannya. Ia mengajukan, lalu harus
--      menebak — membuka halaman izin berkali-kali sampai statusnya berubah.
--      Itu juga sebabnya lonceng di header selama ini hanya hiasan yang selalu
--      berbunyi "belum ada notifikasi".
--
-- Tabel ini melengkapi WA, bukan menggantikannya: WA tetap yang membangunkan
-- orang, notifikasi ini yang membuat riwayatnya bisa dibaca ulang di aplikasi.
--
-- ── KENAPA TEKSNYA DISIMPAN, BUKAN DIRANGKAI SAAT DIBACA ───────────────────
-- Judul & isi ditulis sekali saat kejadiannya, lalu tak pernah dihitung lagi.
-- Alternatifnya — menyimpan hanya `permit_id` lalu men-JOIN saat menampilkan —
-- terlihat lebih rapi tapi salah untuk hal yang sifatnya CATATAN: kalau izinnya
-- kelak dihapus atau diubah, notifikasi yang sudah terkirim ikut berubah bunyi
-- atau lenyap, padahal ia melaporkan sesuatu yang memang benar-benar terjadi.
-- Ini juga yang membuat pembacaannya satu query tanpa join sama sekali.

CREATE TABLE IF NOT EXISTS notifications (
    id          BIGSERIAL PRIMARY KEY,
    -- Penerima. ON DELETE CASCADE: notifikasi milik akun yang dihapus tak punya
    -- arti apa pun dan tak boleh menahan penghapusan akunnya.
    user_id     BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- Jenis kejadian: menentukan ikon & warna di lonceng, bukan isinya.
    --   izin_baru       → ada pengajuan masuk (untuk wali kelas & admin)
    --   izin_disetujui  → keputusan (untuk santri)
    --   izin_ditolak    → keputusan (untuk santri)
    kind        TEXT        NOT NULL,
    title       TEXT        NOT NULL,
    body        TEXT        NOT NULL,
    -- Tujuan saat notifikasinya diketuk. NULL = tak ke mana-mana.
    link        TEXT,
    -- NULL = belum dibaca. Waktu, bukan boolean: "kapan dibaca" gratis didapat
    -- dan sesekali berguna, sementara boolean membuang informasi itu selamanya.
    read_at     TIMESTAMPTZ,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Satu-satunya pola baca daftar: notifikasi SATU orang, terbaru dulu.
-- Kolomnya diurut persis seperti itu supaya query feed jadi index scan murni,
-- tanpa sort terpisah.
CREATE INDEX IF NOT EXISTS idx_notifications_user_baru
    ON notifications (user_id, created_at DESC);

-- Penghitung lonceng dijalankan di SETIAP pemuatan halaman oleh setiap orang
-- yang sedang masuk — jadi ia query terpanas di tabel ini, dan yang dihitung
-- selalu himpunan kecil (yang belum dibaca) di dalam tabel yang lama-lama
-- besar. Index PARSIAL hanya memuat baris yang belum dibaca: ia tetap mungil
-- meski tabelnya tumbuh, karena baris yang sudah dibaca KELUAR dari index
-- begitu `read_at` terisi.
CREATE INDEX IF NOT EXISTS idx_notifications_belum_dibaca
    ON notifications (user_id)
    WHERE read_at IS NULL;
