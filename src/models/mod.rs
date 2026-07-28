//! models — DTO/struct bersama seluruh lapisan (web, repository, service,
//! handler), dipisah per-domain. Dikompilasi untuk SEMUA target (native + WASM):
//! jangan taruh tipe yang menarik dep server-only (tokio/axum/postgres) di sini.

pub mod admin;
pub mod attendance;
pub mod auth;
pub mod books;
pub mod calendar;
pub mod dashboard;
pub mod gallery;
pub mod hafalan;
pub mod kelas;
pub mod laporan;
pub mod materials;
pub mod parent;
pub mod rekap;
pub mod santri;
pub mod schedule;

pub use admin::*;
pub use attendance::*;
pub use auth::*;
pub use books::*;
pub use calendar::*;
pub use dashboard::*;
pub use gallery::*;
pub use hafalan::*;
pub use kelas::*;
pub use laporan::*;
pub use materials::*;
pub use parent::*;
pub use rekap::*;
pub use santri::*;
pub use schedule::*;
