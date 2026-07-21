//! models — DTO/struct bersama seluruh lapisan (web, repository, service,
//! handler), dipisah per-domain. Dikompilasi untuk SEMUA target (native + WASM):
//! jangan taruh tipe yang menarik dep server-only (tokio/axum/postgres) di sini.

pub mod attendance;
pub mod auth;
pub mod dashboard;
pub mod hafalan;
pub mod kelas;
pub mod laporan;
pub mod parent;
pub mod santri;
pub mod schedule;

pub use attendance::*;
pub use auth::*;
pub use dashboard::*;
pub use hafalan::*;
pub use kelas::*;
pub use laporan::*;
pub use parent::*;
pub use santri::*;
pub use schedule::*;
