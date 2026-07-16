//! state.rs — AppState bersama (server-only): pool DB + JwtService.
//! Pola sama e-ticketing: JwtService (key pre-computed) hidup di AppState.

use deadpool_postgres::Pool;

use crate::auth::JwtService;

pub struct AppState {
    pub pool: Pool,
    pub jwt: JwtService,
}

impl AppState {
    pub fn new(pool: Pool, jwt: JwtService) -> Self {
        Self { pool, jwt }
    }
}
