use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use shared::auth::{encode_access_token, generate_refresh_token, hash_secret, REFRESH_TOKEN_TTL_SECS};
use shared::db::{self, PgPool};
use shared::error::AppError;
use time::OffsetDateTime;
use uuid::Uuid;
use serde::Serialize;

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

pub fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hash: {e}")))
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| AppError::Internal(format!("hash parse: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn validate_email(email: &str) -> Result<(), AppError> {
    let at = email.find('@').ok_or_else(|| AppError::Validation("invalid email".into()))?;
    if at == 0 || at == email.len() - 1 || !email[at + 1..].contains('.') {
        return Err(AppError::Validation("invalid email".into()));
    }
    Ok(())
}

pub fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::Validation(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

pub async fn issue_auth_response(
    pool: &PgPool,
    user_id: Uuid,
    email: &str,
    secret: &str,
) -> Result<AuthResponse, AppError> {
    let access_token = encode_access_token(user_id, email, secret)?;
    let refresh_raw = generate_refresh_token();
    let refresh_hash = hash_secret(&refresh_raw);
    let expires_at =
        OffsetDateTime::now_utc() + time::Duration::seconds(REFRESH_TOKEN_TTL_SECS);
    db::refresh_token::create(pool, user_id, &refresh_hash, expires_at).await?;
    Ok(AuthResponse {
        access_token,
        refresh_token: refresh_raw,
        token_type: "Bearer",
        expires_in: shared::auth::ACCESS_TOKEN_TTL_SECS,
    })
}
