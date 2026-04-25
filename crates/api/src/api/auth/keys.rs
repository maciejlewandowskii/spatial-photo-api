use rocket::serde::json::Json;
use rocket::State;
use rocket::http::Status;
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::auth::{generate_api_key, hash_secret};
use time::OffsetDateTime;
use uuid::Uuid;
use serde::{Deserialize, Serialize};

use crate::error::ApiError;
use crate::guards::AuthUser;

#[derive(Deserialize)]
pub struct CreateKeyRequest {
    pub name: String,
    pub expires_at: Option<i64>,
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

#[post("/keys", data = "<body>")]
pub async fn create(
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
pub async fn list(
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
pub async fn delete(
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
