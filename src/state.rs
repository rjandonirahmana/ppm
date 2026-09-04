//! state.rs — AppState bersama (server-only): pool DB + JwtService + storage
//! + bus SSE ruang live. Pola sama e-ticketing: JwtService pre-computed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use deadpool_postgres::Pool;
use redis::aio::ConnectionManager;
use tokio::sync::broadcast;

use crate::auth::JwtService;
use crate::config::WahaConfig;
use crate::service::storage::StorageService;

pub struct AppState {
    pub pool: Pool,
    pub jwt: JwtService,
    /// Rahasia mentah yang sama dengan yang dipakai `jwt`.
    ///
    /// `JwtService` menyimpan kunci dalam bentuk yang sudah disiapkan dan tak
    /// bisa dibaca balik, sedangkan langganan kalender butuh menurunkan token
    /// URL-nya sendiri (`service::ics`) dari rahasia yang sama. Disimpan di
    /// sini supaya tak ada rahasia KEDUA yang harus ikut dikelola operator —
    /// dan supaya mengganti `JWT_SECRET` sekaligus mencabut semua langganan.
    pub jwt_secret: String,
    /// RustFS untuk rekaman siaran; None = simpan lokal (RUSTFS_ENDPOINT kosong).
    pub storage: Option<Arc<StorageService>>,
    /// Redis: link undangan registrasi + pending registration/OTP (lihat
    /// service/registration.rs). `ConnectionManager` sudah Clone murah &
    /// auto-reconnect — dipakai langsung, bukan lewat pool.
    pub redis: ConnectionManager,
    /// HTTP client dipakai ulang (pool koneksi) utk panggil WAHA — dibangun
    /// SEKALI, pola sama StorageService/JwtService.
    pub http: reqwest::Client,
    pub waha: Arc<WahaConfig>,
    /// Bus sinyal per-sesi utk SSE /api/live-events/{id}: chat/status/rekaman
    /// berubah → send(()) → klien refetch. Payload kosong (klien fetch sendiri)
    /// → tak ada state basi. Pengganti polling 4 dtk (audit Jul 2026 poin 3).
    live_bus: Mutex<HashMap<i64, broadcast::Sender<()>>>,
}

impl AppState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        pool: Pool,
        jwt: JwtService,
        jwt_secret: String,
        storage: Option<Arc<StorageService>>,
        redis: ConnectionManager,
        http: reqwest::Client,
        waha: Arc<WahaConfig>,
    ) -> Self {
        Self {
            pool,
            jwt,
            jwt_secret,
            storage,
            redis,
            http,
            waha,
            live_bus: Mutex::new(HashMap::new()),
        }
    }
}

/// Buang entri bus yang tak punya satu pun pendengar tersisa.
///
/// Entri tanpa pendengar = sesi yang sudah ditinggalkan. Termasuk sesi yang
/// sebentar lagi disubscribe ulang — dan itu tak apa: pemanggilnya akan
/// membuatkan kanal baru, dan kanal lama yang tak didengarkan siapa pun memang
/// tak menyimpan apa pun yang perlu diselamatkan.
///
/// Fungsi bebas, bukan method, semata supaya ia bisa diuji tanpa membangun
/// `AppState` utuh — yang menuntut pool Postgres, Redis, dan klien HTTP.
fn sapu_bus(map: &mut HashMap<i64, broadcast::Sender<()>>) {
    map.retain(|_, tx| tx.receiver_count() > 0);
}

impl AppState {
    /// Kunci `live_bus`, TAHAN keracunan (poisoning).
    ///
    /// `lock().unwrap()` akan mengubah satu panik yang pernah terjadi sambil
    /// memegang kunci ini menjadi kerusakan PERMANEN: setiap pemanggil
    /// berikutnya ikut panik, sehingga seluruh ruang live mati sampai proses
    /// di-restart. Isi map ini hanya kumpulan pengirim broadcast — tak ada
    /// invarian yang bisa rusak setengah jalan — jadi memakai kembali data yang
    /// "teracuni" itu aman dan jauh lebih baik daripada mati beruntun.
    fn bus(&self) -> std::sync::MutexGuard<'_, HashMap<i64, broadcast::Sender<()>>> {
        self.live_bus.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Daftar sebagai pendengar perubahan sesi `session_id`.
    ///
    /// ── KENAPA MENYAPU DI SINI ─────────────────────────────────────────────
    /// Pembersihan di `notify_live` saja TIDAK CUKUP, meski komentarnya dulu
    /// mengklaim begitu. Ia hanya membuang entri saat ada yang mencoba
    /// mengirim ke sesi yang pendengarnya sudah habis — dan sesi yang sudah
    /// selesai justru tak pernah dikirimi apa-apa lagi. Jadi setiap sesi yang
    /// pernah dibuka seseorang meninggalkan satu `broadcast::Sender` di peta
    /// ini SELAMANYA.
    ///
    /// Sesi lahir per kelas per tanggal. Dalam satu tahun ajaran itu ribuan id,
    /// dan proses ini dirancang hidup terus tanpa restart — persis bentuk
    /// kebocoran yang tak pernah terlihat di pengujian sehari dan baru terasa
    /// setelah berbulan-bulan.
    ///
    /// Sapuannya ditaruh di sini, bukan di penyapu berkala: satu-satunya yang
    /// MENAMBAH isi peta ini adalah fungsi ini sendiri, jadi di sinilah tempat
    /// yang menjamin peta tak pernah lebih besar dari jumlah sesi yang benar-
    /// benar sedang didengarkan. Biayanya O(n) atas n yang kecil, dan hanya
    /// terjadi saat seseorang membuka ruang sesi — bukan di jalur panas.
    pub fn subscribe_live(&self, session_id: i64) -> broadcast::Receiver<()> {
        let mut map = self.bus();
        sapu_bus(&mut map);
        map.entry(session_id).or_insert_with(|| broadcast::channel(16).0).subscribe()
    }

    /// Beri tahu semua pendengar sesi `session_id` (best-effort).
    ///
    /// Entri yang pendengarnya sudah habis dibuang di sini juga — itu jalur
    /// tercepat untuk sesi yang ditinggalkan sementara siarannya masih berjalan.
    /// Yang menjamin petanya tak tumbuh tanpa batas tetap sapuan di
    /// [`Self::subscribe_live`]; lihat catatannya di sana.
    pub fn notify_live(&self, session_id: i64) {
        let mut map = self.bus();
        if let Some(tx) = map.get(&session_id) {
            if tx.send(()).is_err() {
                map.remove(&session_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bangun peta bus berisi `n` sesi, dan KEMBALIKAN pendengarnya.
    ///
    /// Pendengarnya harus ikut dipegang pemanggil: begitu `Receiver` di-drop,
    /// `receiver_count()` turun jadi nol — dan itulah persis yang sedang diuji.
    fn bus_dgn(n: i64) -> (HashMap<i64, broadcast::Sender<()>>, Vec<broadcast::Receiver<()>>) {
        let mut map = HashMap::new();
        let mut rx = Vec::new();
        for id in 0..n {
            let (tx, r) = broadcast::channel(16);
            map.insert(id, tx);
            rx.push(r);
        }
        (map, rx)
    }

    /// Sesi yang MASIH didengarkan tak boleh ikut tersapu — menyapunya berarti
    /// memutus SSE orang yang sedang membuka ruang sesi.
    #[test]
    fn sesi_yang_masih_didengarkan_dipertahankan() {
        let (mut map, _rx) = bus_dgn(3);
        sapu_bus(&mut map);
        assert_eq!(map.len(), 3);
    }

    /// Inti perbaikan kebocoran: entri yang pendengarnya sudah pergi DIBUANG.
    ///
    /// Tanpa ini, tiap sesi yang pernah dibuka seseorang meninggalkan satu
    /// `broadcast::Sender` selamanya — dan sesi lahir per kelas per tanggal,
    /// jadi ribuan per tahun ajaran di proses yang tak pernah restart.
    #[test]
    fn sesi_yang_ditinggalkan_dibuang() {
        let (mut map, rx) = bus_dgn(3);
        drop(rx);
        sapu_bus(&mut map);
        assert!(map.is_empty(), "entri tanpa pendengar harus dibuang");
    }

    /// Campuran: hanya yang mati yang hilang.
    #[test]
    fn hanya_yang_mati_yang_dibuang() {
        let (mut map, mut rx) = bus_dgn(3);
        rx.remove(1); // pendengar sesi 1 pergi; sesi 0 & 2 masih ada
        sapu_bus(&mut map);
        assert_eq!(map.len(), 2);
        assert!(map.contains_key(&0) && map.contains_key(&2));
        assert!(!map.contains_key(&1));
    }

    /// Satu sesi bisa punya BEBERAPA pendengar — satu ruang pengajian diikuti
    /// banyak santri. Perginya satu orang tak boleh membuang kanal yang masih
    /// dipakai yang lain.
    #[test]
    fn satu_pendengar_pergi_tak_membuang_kanal_bersama() {
        let mut map = HashMap::new();
        let (tx, rx1) = broadcast::channel::<()>(16);
        let rx2 = tx.subscribe();
        map.insert(7i64, tx);

        drop(rx1);
        sapu_bus(&mut map);
        assert_eq!(map.len(), 1, "masih ada satu pendengar");

        drop(rx2);
        sapu_bus(&mut map);
        assert!(map.is_empty(), "pendengar terakhir pergi → entri dibuang");
    }

    /// Menyapu peta kosong aman dan tak melakukan apa-apa.
    #[test]
    fn menyapu_peta_kosong_aman() {
        let mut map: HashMap<i64, broadcast::Sender<()>> = HashMap::new();
        sapu_bus(&mut map);
        assert!(map.is_empty());
    }

    /// Menyapu berkali-kali memberi hasil yang sama — sapuan dipanggil pada
    /// SETIAP `subscribe_live`, jadi ia harus idempoten.
    #[test]
    fn sapuan_idempoten() {
        let (mut map, mut rx) = bus_dgn(4);
        rx.truncate(2); // dua pendengar terakhir pergi
        sapu_bus(&mut map);
        let sesudah_sekali = map.len();
        sapu_bus(&mut map);
        sapu_bus(&mut map);
        assert_eq!(map.len(), sesudah_sekali);
        assert_eq!(sesudah_sekali, 2);
    }
}
