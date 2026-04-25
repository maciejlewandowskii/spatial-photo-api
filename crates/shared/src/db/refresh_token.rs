use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RefreshToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: OffsetDateTime,
    pub created_at: OffsetDateTime,
}

pub async fn create(
    pool: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    expires_at: OffsetDateTime,
) -> Result<RefreshToken, AppError> {
    sqlx::query_as::<_, RefreshToken>(
        "INSERT INTO refresh_tokens (user_id, token_hash, expires_at)
         VALUES ($1, $2, $3) RETURNING *",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)
}

/// Finds a non-expired refresh token by its hash.
pub async fn find_by_hash(
    pool: &PgPool,
    token_hash: &str,
) -> Result<Option<RefreshToken>, AppError> {
    sqlx::query_as::<_, RefreshToken>(
        "SELECT * FROM refresh_tokens WHERE token_hash = $1 AND expires_at > NOW()",
    )
    .bind(token_hash)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

pub async fn delete(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("DELETE FROM refresh_tokens WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}

pub async fn delete_expired(pool: &PgPool) -> Result<u64, AppError> {
    let result = sqlx::query("DELETE FROM refresh_tokens WHERE expires_at <= NOW()")
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(result.rows_affected())
}
