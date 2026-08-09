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
    pub fn subscribe_live(&self, session_id: i64) -> broadcast::Receiver<()> {
        let mut map = self.bus();
        map.entry(session_id).or_insert_with(|| broadcast::channel(16).0).subscribe()
    }

    /// Beri tahu semua pendengar sesi `session_id` (best-effort). Entry tanpa
    /// pendengar dibersihkan di sini → map tak tumbuh melewati sesi yang aktif.
    pub fn notify_live(&self, session_id: i64) {
        let mut map = self.bus();
        if let Some(tx) = map.get(&session_id) {
            if tx.send(()).is_err() {
                map.remove(&session_id);
            }
        }
    }
}
