//! auth.rs — JwtService (server-only), pola SAMA dengan e-ticketing utils/jwt.rs:
//! EncodingKey + DecodingKey di-pre-compute dari secret SEKALI (di AppState) —
//! bukan dibuat ulang tiap sign()/verify() (hemat alokasi + CPU).
//!
//! Token disimpan di cookie HttpOnly `ppm_token` (SameSite=Lax, 7 hari) —
//! umur token JWT sendiri 100 hari (sama seperti e-ticketing).

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

use crate::models::Claims;

pub const COOKIE_NAME: &str = "ppm_token";
/// Umur sesi (Max-Age cookie) = umur token, supaya cookie tak hidup lebih lama
/// dari tokennya. Dikombinasikan dgn SLIDING refresh (get_session men-sign ulang
/// token + set ulang cookie tiap kunjungan): pengguna aktif tak pernah diminta
/// login ulang, tapi yang berhenti memakai aplikasi otomatis keluar setelah
/// TOKEN_DAYS.
pub const SESSION_SECS: i64 = TOKEN_DAYS * 24 * 3600;

/// Umur token JWT: 30 hari.
///
/// Dulu 36.500 hari (100 tahun) — praktis abadi. Itu berarti sesi TIDAK BISA
/// DICABUT: pengguna yang dinonaktifkan, diturunkan perannya, atau yang HP-nya
/// hilang tetap punya akses penuh selamanya, karena `require_session` hanya
/// memeriksa tanda tangan token dan tak pernah menanyakan keadaan terkini ke
/// database.
///
/// 30 hari membatasi kerusakan itu tanpa menambah query per request. Ini BUKAN
/// pencabutan seketika — perubahan peran/status masih butuh sampai 30 hari untuk
/// benar-benar berlaku pada sesi yang sudah berjalan. Pencabutan seketika butuh
/// `token_version` atau cache Redis; belum dikerjakan.
pub const TOKEN_DAYS: i64 = 30;

/// Kenapa sebuah token ditolak.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenError {
    /// Umur token habis (wajar) — minta login ulang.
    Expired,
    /// Tanda tangan salah / bentuk rusak — tak seharusnya terjadi.
    Invalid,
}

impl TokenError {
    /// Penanda yang dibaca lapisan web & klien. "unauth" DIPERTAHANKAN untuk
    /// token cacat karena halaman-halaman sudah mencocokkan kata itu untuk
    /// mengalihkan ke /login.
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenError::Expired => "session_expired",
            TokenError::Invalid => "unauth",
        }
    }
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::error::Error for TokenError {}

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

    /// Verifikasi token. Kedaluwarsa DIBEDAKAN dari token cacat/palsu: yang
    /// pertama wajar terjadi (sesi 30 hari habis) dan pengguna cukup diminta
    /// masuk lagi; yang kedua patut dicurigai dan layak masuk log peringatan.
    pub fn verify(&self, token: &str) -> Result<Claims, TokenError> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        decode::<Claims>(token, &self.dec, &validation)
            .map(|d| d.claims)
            .map_err(|e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => TokenError::Expired,
                _ => {
                    tracing::warn!("JWT verify failed: {:?}", e);
                    TokenError::Invalid
                }
            })
    }
}
