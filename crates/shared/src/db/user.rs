use serde::Serialize;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    #[serde(skip)]
    pub password_hash: String,
    pub tokens: i32,
    pub created_at: OffsetDateTime,
}

pub async fn create(pool: &PgPool, email: &str, password_hash: &str) -> Result<User, AppError> {
    sqlx::query_as::<_, User>(
        "INSERT INTO users (email, password_hash) VALUES ($1, $2) RETURNING *",
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db) if db.constraint() == Some("users_email_key") => {
            AppError::Validation("email already registered".into())
        }
        other => AppError::Database(other),
    })
}

pub async fn find_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, AppError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
        .bind(email)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, AppError> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
}

/// Atomically deducts `amount` tokens. Returns `None` if balance is insufficient.
pub async fn deduct_tokens(
    pool: &PgPool,
    id: Uuid,
    amount: i32,
) -> Result<Option<i32>, AppError> {
    sqlx::query_scalar::<_, i32>(
        "UPDATE users SET tokens = tokens - $1 WHERE id = $2 AND tokens >= $1 RETURNING tokens",
    )
    .bind(amount)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(AppError::Database)
}

pub async fn refund_tokens(pool: &PgPool, id: Uuid, amount: i32) -> Result<(), AppError> {
    sqlx::query("UPDATE users SET tokens = tokens + $1 WHERE id = $2")
        .bind(amount)
        .bind(id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}
