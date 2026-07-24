//! service — Logika bisnis (server-only), dipisah per-domain.
//! Lapisan: handler (web/api, device_api) → service → repository → Postgres.

pub mod admin;
pub mod attendance;
pub mod auth;
pub mod books;
pub mod calendar;
pub mod dashboard;
pub mod export;
pub mod fmt;
pub mod gate;
pub mod hafalan;
pub mod kelas;
pub mod laporan;
pub mod materials;
pub mod parent;
pub mod permits;
pub mod recording;
pub mod registration;
pub mod santri;
pub mod sessions;
pub mod storage;
