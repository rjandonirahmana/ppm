//! service — Logika bisnis (server-only), dipisah per-domain.
//! Lapisan: handler (web/api, device_api) → service → repository → Postgres.

pub mod attendance;
pub mod auth;
pub mod dashboard;
pub mod fmt;
pub mod kelas;
pub mod parent;
pub mod santri;
pub mod sessions;
