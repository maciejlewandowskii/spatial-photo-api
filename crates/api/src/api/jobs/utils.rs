use std::io::Cursor;
use image::ImageReader;
use rocket::data::{Data, ToByteUnit};
use rocket::form::FromForm;
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::jobs::{JobMessage, JobOptions, JobType};
use shared::queue;
use shared::storage;
use uuid::Uuid;
use serde::{Deserialize, Serialize};
use validator::Validate;
use rocket_okapi::okapi::schemars::{self, JsonSchema};

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::{S3State, SqsState, MAX_IMAGE_BYTES, MAX_DIMENSION};

#[derive(Deserialize, Default, Validate, FromForm, JsonSchema)]
pub struct ConvertOptions {
    #[validate(range(min = 1, max = 100))]
    pub max_disparity: Option<u32>,
    pub model: Option<String>,
    #[validate(range(min = 1.0, max = 180.0))]
    pub horizontal_fov: Option<f32>,
    #[validate(range(min = 1.0, max = 1000.0))]
    pub baseline_mm: Option<f32>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobResponse {
    pub id: String,
    pub status: String,
    pub job_type: String,
    pub tokens_charged: i32,
    pub progress: i32,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_url: Option<String>,
    pub expires_at: Option<i64>,
    pub completed_at: Option<i64>,
}

pub struct ValidatedImage {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
    pub content_type: &'static str,
}

pub fn validate_image_bytes(raw: Vec<u8>) -> Result<ValidatedImage, AppError> {
    if raw.len() > MAX_IMAGE_BYTES as usize {
         return Err(AppError::Validation(format!(
            "image exceeds {:.0} MB limit",
            MAX_IMAGE_BYTES as f64 / 1_048_576.0
        )));
    }

    let fmt = image::guess_format(&raw)
        .map_err(|_| AppError::Validation("unrecognised image format".into()))?;

    let (width, height) = ImageReader::new(Cursor::new(&raw))
        .with_guessed_format()
        .map_err(|_| AppError::Validation("could not read image header".into()))?
        .into_dimensions()
        .map_err(|_| AppError::Validation("could not read image dimensions".into()))?;

    if width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(AppError::Validation(format!(
            "image dimensions {width}×{height} exceed maximum {MAX_DIMENSION}×{MAX_DIMENSION}"
        )));
    }

    let (extension, content_type) = match fmt {
        image::ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        image::ImageFormat::Png => ("png", "image/png"),
        image::ImageFormat::WebP => ("webp", "image/webp"),
        image::ImageFormat::Tiff => ("tiff", "image/tiff"),
        image::ImageFormat::Bmp => ("bmp", "image/bmp"),
        image::ImageFormat::Gif => ("gif", "image/gif"),
        other => {
            return Err(AppError::Validation(format!(
                "unsupported format: {other:?}"
            )))
        }
    };

    Ok(ValidatedImage {
        bytes: raw,
        extension,
        content_type,
    })
}

pub async fn read_and_validate_image(data: Data<'_>) -> Result<ValidatedImage, AppError> {
    let bytes = data
        .open(MAX_IMAGE_BYTES.bytes())
        .into_bytes()
        .await
        .map_err(|e| AppError::Internal(format!("read upload: {e}")))?;

    if !bytes.is_complete() {
        return Err(AppError::Validation(format!(
            "image exceeds {:.0} MB limit",
            MAX_IMAGE_BYTES as f64 / 1_048_576.0
        )));
    }

    validate_image_bytes(bytes.into_inner())
}

pub async fn dispatch_single_image_from_bytes(
    bytes: Vec<u8>,
    options: ConvertOptions,
    job_type: JobType,
    user: &AuthUser,
    pool: &PgPool,
    s3: &S3State,
    sqs: &SqsState,
) -> Result<JobResponse, ApiError> {
    let validated = validate_image_bytes(bytes)?;

    let cost = job_type.token_cost();
    db::user::deduct_tokens(pool, user.user_id, cost)
        .await?
        .ok_or(AppError::InsufficientTokens)?;

    let job_options = JobOptions {
        max_disparity: options.max_disparity,
        model: options.model,
        horizontal_fov: options.horizontal_fov,
        baseline_mm: options.baseline_mm,
    };

    let job_id = Uuid::new_v4();
    let input_key = format!("uploads/{job_id}/input.{}", validated.extension);

    let job = db::job::create(
        pool,
        db::job::CreateJob {
            user_id: user.user_id,
            job_type: &job_type,
            tokens_charged: cost,
            input_s3_key: Some(&input_key),
            left_s3_key: None,
            right_s3_key: None,
            depth_s3_key: None,
            options: serde_json::to_value(&job_options).map_err(|e| AppError::Internal(format!("serialize options: {e}")))?,
        },
    )
    .await?;

    if let Err(e) = storage::upload(&s3.client, &s3.bucket, &input_key, validated.bytes, validated.content_type).await {
        let _ = db::job::set_status(pool, job.id, &shared::jobs::JobStatus::Failed, Some(&e.to_string())).await;
        return Err(e.into());
    }

    let msg = JobMessage {
        job_id: job.id,
        job_type,
        user_id: user.user_id,
        options: job_options,
    };

    if let Err(e) = queue::enqueue(&sqs.client, &sqs.queue_url, &msg).await {
        let _ = db::job::set_status(pool, job.id, &shared::jobs::JobStatus::Failed, Some(&e.to_string())).await;
        return Err(e.into());
    }

    Ok(job_to_response(job, None))
}

pub async fn dispatch_single_image(
    data: Data<'_>,
    options: ConvertOptions,
    job_type: JobType,
    user: &AuthUser,
    pool: &PgPool,
    s3: &S3State,
    sqs: &SqsState,
) -> Result<JobResponse, ApiError> {
    let validated = read_and_validate_image(data).await?;

    let cost = job_type.token_cost();
    db::user::deduct_tokens(pool, user.user_id, cost)
        .await?
        .ok_or(AppError::InsufficientTokens)?;

    let job_options = JobOptions {
        max_disparity: options.max_disparity,
        model: options.model,
        horizontal_fov: options.horizontal_fov,
        baseline_mm: options.baseline_mm,
    };

    let job_id = Uuid::new_v4();
    let input_key = format!("uploads/{job_id}/input.{}", validated.extension);

    let job = db::job::create(
        pool,
        db::job::CreateJob {
            user_id: user.user_id,
            job_type: &job_type,
            tokens_charged: cost,
            input_s3_key: Some(&input_key),
            left_s3_key: None,
            right_s3_key: None,
            depth_s3_key: None,
            options: serde_json::to_value(&job_options).map_err(|e| AppError::Internal(format!("serialize options: {e}")))?,
        },
    )
    .await?;

    if let Err(e) = storage::upload(&s3.client, &s3.bucket, &input_key, validated.bytes, validated.content_type).await {
        let _ = db::job::set_status(pool, job.id, &shared::jobs::JobStatus::Failed, Some(&e.to_string())).await;
        return Err(e.into());
    }

    let msg = JobMessage {
        job_id: job.id,
        job_type,
        user_id: user.user_id,
        options: job_options,
    };

    if let Err(e) = queue::enqueue(&sqs.client, &sqs.queue_url, &msg).await {
        let _ = db::job::set_status(pool, job.id, &shared::jobs::JobStatus::Failed, Some(&e.to_string())).await;
        return Err(e.into());
    }

    Ok(job_to_response(job, None))
}

pub fn job_to_response(job: shared::db::job::Job, download_url: Option<String>) -> JobResponse {
    JobResponse {
        id: job.id.to_string(),
        status: job.status,
        job_type: job.job_type,
        tokens_charged: job.tokens_charged,
        progress: job.progress,
        error: job.error,
        download_url,
        created_at: job.created_at.unix_timestamp(),
        expires_at: job.expires_at.map(|t| t.unix_timestamp()),
        completed_at: job.completed_at.map(|t| t.unix_timestamp()),
    }
}
