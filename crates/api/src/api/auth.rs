use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use shared::auth::{
    encode_access_token, generate_api_key, generate_refresh_token, hash_secret,
    REFRESH_TOKEN_TTL_SECS,
};
use shared::db::{self, PgPool};
use shared::error::AppError;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::ApiError;
use crate::guards::{AuthUser, JwtSecret};

// ── Request/response types ──────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[derive(Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub expires_at: Option<i64>,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub tokens: i32,
    pub created_at: i64,
}

#[derive(Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub expires_at: Option<i64>,
    pub last_used_at: Option<i64>,
    pub created_at: i64,
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn hash_password(password: &str) -> Result<String, AppError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Internal(format!("password hash: {e}")))
}

fn verify_password(password: &str, hash: &str) -> Result<bool, AppError> {
    let parsed =
        PasswordHash::new(hash).map_err(|e| AppError::Internal(format!("hash parse: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn validate_email(email: &str) -> Result<(), AppError> {
    let at = email.find('@').ok_or_else(|| AppError::Validation("invalid email".into()))?;
    if at == 0 || at == email.len() - 1 || !email[at + 1..].contains('.') {
        return Err(AppError::Validation("invalid email".into()));
    }
    Ok(())
}

fn validate_password(password: &str) -> Result<(), AppError> {
    if password.len() < 8 {
        return Err(AppError::Validation(
            "password must be at least 8 characters".into(),
        ));
    }
    Ok(())
}

async fn issue_auth_response(
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

// ── Handlers ────────────────────────────────────────────────────────────────

use rocket::http::{Cookie, CookieJar, Status};
use rocket::response::Redirect;

#[post("/register", data = "<body>")]
pub async fn register(
    body: Json<RegisterRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
    cookies: &CookieJar<'_>,
) -> Result<Json<AuthResponse>, ApiError> {
    validate_email(&body.email)?;
    validate_password(&body.password)?;
    let hash = hash_password(&body.password)?;
    let user = db::user::create(pool, &body.email.to_lowercase(), &hash).await?;
    let resp = issue_auth_response(pool, user.id, &user.email, &jwt.0).await?;
    
    cookies.add(Cookie::build(("access_token", resp.access_token.clone()))
        .path("/")
        .http_only(true)
        .build());
    
    Ok(Json(resp))
}

#[post("/login", data = "<body>")]
pub async fn login(
    body: Json<LoginRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
    cookies: &CookieJar<'_>,
) -> Result<Json<AuthResponse>, ApiError> {
    let user = db::user::find_by_email(pool, &body.email.to_lowercase())
        .await?
        .ok_or(AppError::InvalidCredentials)?;

    if !verify_password(&body.password, &user.password_hash)? {
        return Err(AppError::InvalidCredentials.into());
    }

    let resp = issue_auth_response(pool, user.id, &user.email, &jwt.0).await?;
    
    cookies.add(Cookie::build(("access_token", resp.access_token.clone()))
        .path("/")
        .http_only(true)
        .build());
    
    Ok(Json(resp))
}

#[post("/refresh", data = "<body>")]
pub async fn refresh(
    body: Json<RefreshRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
) -> Result<Json<AccessTokenResponse>, ApiError> {
    let hash = hash_secret(&body.refresh_token);
    let rt = db::refresh_token::find_by_hash(pool, &hash)
        .await?
        .ok_or(AppError::InvalidToken)?;

    let user = db::user::find_by_id(pool, rt.user_id)
        .await?
        .ok_or(AppError::NotFound("user".into()))?;

    db::refresh_token::delete(pool, rt.id).await?;

    let access_token = encode_access_token(user.id, &user.email, &jwt.0)?;
    Ok(Json(AccessTokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: shared::auth::ACCESS_TOKEN_TTL_SECS,
    }))
}

#[get("/logout")]
pub async fn logout_get(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove(Cookie::from("access_token"));
    Redirect::to("/login")
}

#[post("/logout", data = "<body>")]
pub async fn logout(
    body: Json<LogoutRequest>,
    pool: &State<PgPool>,
    cookies: &CookieJar<'_>,
) -> Result<Status, ApiError> {
    let hash = hash_secret(&body.refresh_token);
    if let Some(rt) = db::refresh_token::find_by_hash(pool, &hash).await? {
        db::refresh_token::delete(pool, rt.id).await?;
    }
    cookies.remove(Cookie::from("access_token"));
    Ok(Status::NoContent)
}

#[get("/me")]
pub async fn me(
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Json<UserResponse>, ApiError> {
    let u = db::user::find_by_id(pool, user.user_id)
        .await?
        .ok_or(AppError::NotFound("user".into()))?;

    Ok(Json(UserResponse {
        id: u.id.to_string(),
        email: u.email,
        tokens: u.tokens,
        created_at: u.created_at.unix_timestamp(),
    }))
}

#[post("/keys", data = "<body>")]
pub async fn create_key(
    body: Json<CreateKeyRequest>,
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Json<ApiKeyResponse>, ApiError> {
    if body.name.is_empty() {
        return Err(AppError::Validation("name is required".into()).into());
    }

    let raw_key = generate_api_key();
    let key_hash = hash_secret(&raw_key);

    let expires_at = body
        .expires_at
        .map(OffsetDateTime::from_unix_timestamp)
        .transpose()
        .map_err(|_| AppError::Validation("invalid expires_at timestamp".into()))?;

    let key = db::api_key::create(pool, user.user_id, &key_hash, &body.name, expires_at).await?;

    Ok(Json(ApiKeyResponse {
        id: key.id.to_string(),
        name: key.name,
        key: Some(raw_key),
        expires_at: key.expires_at.map(|t| t.unix_timestamp()),
        last_used_at: None,
        created_at: key.created_at.unix_timestamp(),
    }))
}

#[get("/keys")]
pub async fn list_keys(
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Json<Vec<ApiKeyResponse>>, ApiError> {
    let keys = db::api_key::list_for_user(pool, user.user_id).await?;
    let resp = keys
        .into_iter()
        .map(|k| ApiKeyResponse {
            id: k.id.to_string(),
            name: k.name,
            key: None,
            expires_at: k.expires_at.map(|t| t.unix_timestamp()),
            last_used_at: k.last_used_at.map(|t| t.unix_timestamp()),
            created_at: k.created_at.unix_timestamp(),
        })
        .collect();
    Ok(Json(resp))
}

#[delete("/keys/<id>")]
pub async fn delete_key(
    id: &str,
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Status, ApiError> {
    let key_id = id
        .parse::<Uuid>()
        .map_err(|_| AppError::Validation("invalid key id".into()))?;

    let deleted = db::api_key::delete(pool, key_id, user.user_id).await?;
    if deleted {
        Ok(Status::NoContent)
    } else {
        Err(AppError::NotFound("api key".into()).into())
    }
}
