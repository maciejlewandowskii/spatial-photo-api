use serde::Serialize;
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::AppError;
use crate::jobs::{JobStatus, JobType};

#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
pub struct Job {
    pub id: Uuid,
    pub user_id: Uuid,
    pub job_type: String,
    pub status: String,
    pub tokens_charged: i32,
    pub input_s3_key: Option<String>,
    pub left_s3_key: Option<String>,
    pub right_s3_key: Option<String>,
    pub depth_s3_key: Option<String>,
    pub output_s3_key: Option<String>,
    pub progress: i32,
    pub error: Option<String>,
    pub options: JsonValue,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub expires_at: Option<OffsetDateTime>,
    pub completed_at: Option<OffsetDateTime>,
}

pub struct CreateJob<'a> {
    pub user_id: Uuid,
    pub job_type: &'a JobType,
    pub tokens_charged: i32,
    pub input_s3_key: Option<&'a str>,
    pub left_s3_key: Option<&'a str>,
    pub right_s3_key: Option<&'a str>,
    pub depth_s3_key: Option<&'a str>,
    pub options: JsonValue,
}

pub async fn create(pool: &PgPool, params: CreateJob<'_>) -> Result<Job, AppError> {
    let expires_at = OffsetDateTime::now_utc() + time::Duration::hours(25);
    sqlx::query_as::<_, Job>(
        r#"INSERT INTO jobs
           (user_id, job_type, tokens_charged, input_s3_key, left_s3_key, right_s3_key,
            depth_s3_key, options, expires_at)
           VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9) RETURNING *"#,
    )
    .bind(params.user_id)
    .bind(params.job_type.as_str())
    .bind(params.tokens_charged)
    .bind(params.input_s3_key)
    .bind(params.left_s3_key)
    .bind(params.right_s3_key)
    .bind(params.depth_s3_key)
    .bind(params.options)
    .bind(expires_at)
    .fetch_one(pool)
    .await
    .map_err(AppError::Database)
}

pub async fn find_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Job>, AppError> {
    sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = $1")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(AppError::Database)
}

pub async fn list_for_user(
    pool: &PgPool,
    user_id: Uuid,
    limit: i64,
    offset: i64,
) -> Result<Vec<Job>, AppError> {
    sqlx::query_as::<_, Job>(
        "SELECT * FROM jobs WHERE user_id = $1 ORDER BY created_at DESC LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(AppError::Database)
}

pub async fn set_status(
    pool: &PgPool,
    id: Uuid,
    status: &JobStatus,
    error: Option<&str>,
) -> Result<(), AppError> {
    let completed_at = matches!(status, JobStatus::Complete | JobStatus::Failed | JobStatus::Cancelled)
        .then(OffsetDateTime::now_utc);

    sqlx::query(
        "UPDATE jobs SET status = $1, error = $2, updated_at = NOW(), completed_at = $3
         WHERE id = $4",
    )
    .bind(status.as_str())
    .bind(error)
    .bind(completed_at)
    .bind(id)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;
    Ok(())
}

pub async fn set_progress(pool: &PgPool, id: Uuid, progress: i32) -> Result<(), AppError> {
    sqlx::query("UPDATE jobs SET progress = $1, updated_at = NOW() WHERE id = $2")
        .bind(progress)
        .bind(id)
        .execute(pool)
        .await
        .map_err(AppError::Database)?;
    Ok(())
}

pub async fn set_output(pool: &PgPool, id: Uuid, output_s3_key: &str) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE jobs SET output_s3_key = $1, updated_at = NOW() WHERE id = $2",
    )
    .bind(output_s3_key)
    .bind(id)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;
    Ok(())
}

pub async fn cancel(pool: &PgPool, id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
    let result = sqlx::query(
        "UPDATE jobs SET status = 'cancelled', updated_at = NOW()
         WHERE id = $1 AND user_id = $2 AND status = 'pending'",
    )
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(AppError::Database)?;
    Ok(result.rows_affected() > 0)
}
