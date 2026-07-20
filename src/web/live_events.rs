//! web/live_events.rs — SSE ruang sesi live (pengganti polling 4 dtk).
//!
//! GET /api/live-events/{id} → stream Server-Sent Events. Event TANPA payload
//! ("u") — hanya penanda "ada perubahan"; klien lalu refetch server-fn seperti
//! biasa (satu jalur data, tak ada state ganda/basi). Sinyal dikirim
//! AppState::notify_live saat: chat masuk, status sesi berubah (mulai/akhiri),
//! rekaman selesai dipindah ke RustFS. EventSource browser auto-reconnect;
//! KeepAlive mencegah proxy/idle timeout memutus koneksi.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::Extension;
use tokio_stream::{wrappers::BroadcastStream, Stream, StreamExt};

use crate::state::AppState;

pub async fn events(
    Extension(state): Extension<Arc<AppState>>,
    Path(session_id): Path<i64>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    super::live_audio::auth(&state, &headers)?;
    let rx = state.subscribe_live(session_id);
    // Lagged (tertinggal >16 sinyal) tetap dipetakan ke event: klien refetch
    // sekali dan kembali sinkron — persis semantik yang diinginkan.
    let stream = BroadcastStream::new(rx).map(|_| Ok(Event::default().data("u")));
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
