use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::error::AppError;

pub const ACCESS_TOKEN_TTL_SECS: u64 = 900;       // 15 min
pub const REFRESH_TOKEN_TTL_SECS: i64 = 2_592_000; // 30 days

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub email: String,
    pub exp: usize,
    pub iat: usize,
}

pub fn encode_access_token(
    user_id: Uuid,
    email: &str,
    secret: &str,
) -> Result<String, AppError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as usize;

    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp: now + ACCESS_TOKEN_TTL_SECS as usize,
        iat: now,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AppError::Internal(format!("JWT encode: {e}")))
}

pub fn decode_access_token(token: &str, secret: &str) -> Result<Claims, AppError> {
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map(|d| d.claims)
    .map_err(|e| {
        use jsonwebtoken::errors::ErrorKind;
        match e.kind() {
            ErrorKind::ExpiredSignature => AppError::TokenExpired,
            _ => AppError::InvalidToken,
        }
    })
}

/// Returns a random opaque refresh token (64 hex chars).
pub fn generate_refresh_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    hex::encode(bytes)
}

/// Returns a new API key with `sp_` prefix (65 chars total).
pub fn generate_api_key() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    format!("sp_{}", hex::encode(bytes))
}

/// SHA-256 hex digest of a secret string.
pub fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hex::encode(hasher.finalize())
}
