//! service — Logika bisnis (server-only), dipisah per-domain.
//! Lapisan: handler (web/api, device_api) → service → repository → Postgres.

pub mod attendance;
pub mod auth;
pub mod dashboard;
pub mod fmt;
pub mod gate;
pub mod hafalan;
pub mod kelas;
pub mod laporan;
pub mod parent;
pub mod recording;
pub mod santri;
pub mod sessions;
pub mod storage;
