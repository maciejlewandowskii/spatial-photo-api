use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub user_id: Uuid,
    #[serde(skip)]
    pub key_hash: String,
    pub name: String,
    pub last_used_at: Option<OffsetDateTime>,
    pub expires_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    key_hash: &str,
    name: &str,
    expires_at: Option<OffsetDateTime>,
) -> Result<ApiKey, AppError> {
    sqlx::query_as::<_, ApiKey>(
        "INSERT INTO api_keys (user_id, key_hash, name, expires_at)
         VALUES ($1, $2, $3, $4) RETURNING *",
    )
    .bind(user_id)
    .bind(key_hash)
    .bind(name)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)
}

/// Looks up an API key by its hash, returns None if not found or expired.
pub async fn find_by_hash(pool: &PgPool, key_hash: &str) -> Result<Option<ApiKey>, AppError> {
    sqlx::query_as::<_, ApiKey>(
        "SELECT * FROM api_keys
         WHERE key_hash = $1
           AND (expires_at IS NULL OR expires_at > NOW())",
    )
    .bind(key_hash)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

pub async fn list_for_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<ApiKey>, AppError> {
    sqlx::query_as::<_, ApiKey>(
        "SELECT * FROM api_keys WHERE user_id = $1 ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

pub async fn delete(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query(
        "DELETE FROM api_keys WHERE id = $1 AND user_id = $2",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;
    Ok(result.rows_affected() > 0)
}

pub async fn touch_last_used(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE api_keys SET last_used_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}
