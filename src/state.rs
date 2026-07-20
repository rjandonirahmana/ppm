//! state.rs — AppState bersama (server-only): pool DB + JwtService + storage
//! + bus SSE ruang live. Pola sama e-ticketing: JwtService pre-computed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use deadpool_postgres::Pool;
use tokio::sync::broadcast;

use crate::auth::JwtService;
use crate::service::storage::StorageService;

pub struct AppState {
    pub pool: Pool,
    pub jwt: JwtService,
    /// RustFS untuk rekaman siaran; None = simpan lokal (RUSTFS_ENDPOINT kosong).
    pub storage: Option<Arc<StorageService>>,
    /// Bus sinyal per-sesi utk SSE /api/live-events/{id}: chat/status/rekaman
    /// berubah → send(()) → klien refetch. Payload kosong (klien fetch sendiri)
    /// → tak ada state basi. Pengganti polling 4 dtk (audit Jul 2026 poin 3).
    live_bus: Mutex<HashMap<i64, broadcast::Sender<()>>>,
}

impl AppState {
    pub fn new(pool: Pool, jwt: JwtService, storage: Option<Arc<StorageService>>) -> Self {
        Self { pool, jwt, storage, live_bus: Mutex::new(HashMap::new()) }
    }

    /// Daftar sebagai pendengar perubahan sesi `session_id`.
    pub fn subscribe_live(&self, session_id: i64) -> broadcast::Receiver<()> {
        let mut map = self.live_bus.lock().unwrap();
        map.entry(session_id).or_insert_with(|| broadcast::channel(16).0).subscribe()
    }

    /// Beri tahu semua pendengar sesi `session_id` (best-effort). Entry tanpa
    /// pendengar dibersihkan di sini → map tak tumbuh melewati sesi yang aktif.
    pub fn notify_live(&self, session_id: i64) {
        let mut map = self.live_bus.lock().unwrap();
        if let Some(tx) = map.get(&session_id) {
            if tx.send(()).is_err() {
                map.remove(&session_id);
            }
        }
    }
}
