//! auth.rs — JwtService (server-only), pola SAMA dengan e-ticketing utils/jwt.rs:
//! EncodingKey + DecodingKey di-pre-compute dari secret SEKALI (di AppState) —
//! bukan dibuat ulang tiap sign()/verify() (hemat alokasi + CPU).
//!
//! Token disimpan di cookie HttpOnly `ppm_token` (SameSite=Lax, 7 hari) —
//! umur token JWT sendiri 100 hari (sama seperti e-ticketing).

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::models::Claims;

pub const COOKIE_NAME: &str = "ppm_token";
/// Umur COOKIE sesi: 400 hari = BATAS MAKSIMUM yang diizinkan browser (Chrome
/// meng-cap Max-Age cookie di 400 hari — tak bisa lebih). Dikombinasikan dgn
/// SLIDING refresh (get_session men-sign ulang token + set ulang cookie setiap
/// kunjungan), sesi efektif TIDAK PERNAH expired selama web pernah dibuka
/// setidaknya sekali dalam 400 hari.
pub const SESSION_SECS: i64 = 400 * 24 * 3600;
/// Umur token JWT: 100 tahun — praktis tidak pernah expired.
pub const TOKEN_DAYS: i64 = 36_500;

#[derive(Clone)]
pub struct JwtService {
    enc: EncodingKey,
    dec: DecodingKey,
}

impl JwtService {
    pub fn new(secret: &str) -> Self {
        Self {
            enc: EncodingKey::from_secret(secret.as_bytes()),
            dec: DecodingKey::from_secret(secret.as_bytes()),
        }
    }

    pub fn sign(&self, user_id: i64, name: &str, phone: &str, role: &str) -> anyhow::Result<String> {
        let claims = Claims {
            user_id,
            role: role.to_string(),
            name: name.to_string(),
            phone: phone.to_string(),
            exp: (chrono::Utc::now() + chrono::Duration::days(TOKEN_DAYS)).timestamp(),
        };
        encode(&Header::default(), &claims, &self.enc).map_err(Into::into)
    }

    pub fn verify(&self, token: &str) -> anyhow::Result<Claims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        decode::<Claims>(token, &self.dec, &validation)
            .map(|d| d.claims)
            .map_err(|e| {
                tracing::warn!("JWT verify failed: {:?}", e);
                e.into()
            })
    }
}
