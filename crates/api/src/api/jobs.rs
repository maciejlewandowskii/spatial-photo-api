use std::io::Cursor;
use std::time::Duration;

use image::ImageReader;
use rocket::data::{Data, ToByteUnit};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::State;
use serde::{Deserialize, Serialize};
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::jobs::{JobMessage, JobOptions, JobType};
use shared::queue;
use shared::storage;
use uuid::Uuid;

use crate::error::ApiError;
use crate::guards::AuthUser;

// ── AWS state ────────────────────────────────────────────────────────────────

pub struct S3State {
    pub client: aws_sdk_s3::Client,
    pub bucket: String,
}

pub struct SqsState {
    pub client: aws_sdk_sqs::Client,
    pub queue_url: String,
}

// ── Limits ───────────────────────────────────────────────────────────────────

const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024; // 10 MB (API Gateway hard limit)
const MAX_DIMENSION: u32 = 8192;

// ── Request / response types ─────────────────────────────────────────────────

#[derive(Deserialize, Default)]
pub struct ConvertOptions {
    pub max_disparity: Option<u32>,
    pub model: Option<String>,
    pub horizontal_fov: Option<f32>,
    pub baseline_mm: Option<f32>,
}

#[derive(Serialize)]
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

#[derive(Serialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobResponse>,
    pub total: usize,
}

// ── Image helpers ─────────────────────────────────────────────────────────────

struct ValidatedImage {
    bytes: Vec<u8>,
    extension: &'static str,
    content_type: &'static str,
}

async fn read_and_validate_image(data: Data<'_>) -> Result<ValidatedImage, AppError> {
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

    let raw = bytes.into_inner();

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
                "unsupported format: {other:?}. Accepted: JPEG, PNG, WebP, TIFF, BMP, GIF"
            )))
        }
    };

    Ok(ValidatedImage {
        bytes: raw,
        extension,
        content_type,
    })
}

// ── Generic dispatch helper ───────────────────────────────────────────────────

async fn dispatch_single_image(
    data: Data<'_>,
    options: ConvertOptions,
    job_type: JobType,
    user: &AuthUser,
    pool: &PgPool,
    s3: &S3State,
    sqs: &SqsState,
) -> Result<Json<JobResponse>, ApiError> {
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

    let create_result = db::job::create(
        pool,
        db::job::CreateJob {
            user_id: user.user_id,
            job_type: &job_type,
            tokens_charged: cost,
            input_s3_key: Some(&input_key),
            left_s3_key: None,
            right_s3_key: None,
            depth_s3_key: None,
            options: serde_json::to_value(&job_options)
                .unwrap_or(serde_json::Value::Object(Default::default())),
        },
    )
    .await;

    let job = match create_result {
        Ok(j) => j,
        Err(e) => {
            let _ = db::user::refund_tokens(pool, user.user_id, cost).await;
            return Err(e.into());
        }
    };

    if let Err(e) = storage::upload(
        &s3.client,
        &s3.bucket,
        &input_key,
        validated.bytes,
        validated.content_type,
    )
    .await
    {
        let _ = db::user::refund_tokens(pool, user.user_id, cost).await;
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
        let _ = db::user::refund_tokens(pool, user.user_id, cost).await;
        let _ = db::job::set_status(pool, job.id, &shared::jobs::JobStatus::Failed, Some(&e.to_string())).await;
        return Err(e.into());
    }

    Ok(Json(job_to_response(job, None)))
}

fn job_to_response(job: shared::db::job::Job, download_url: Option<String>) -> JobResponse {
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

// ── Endpoints ─────────────────────────────────────────────────────────────────

#[post("/convert?<max_disparity>&<model>&<horizontal_fov>&<baseline_mm>", data = "<data>")]
pub async fn submit_convert(
    data: Data<'_>,
    max_disparity: Option<u32>,
    model: Option<String>,
    horizontal_fov: Option<f32>,
    baseline_mm: Option<f32>,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
    sqs: &State<SqsState>,
) -> Result<Json<JobResponse>, ApiError> {
    dispatch_single_image(
        data,
        ConvertOptions { max_disparity, model, horizontal_fov, baseline_mm },
        JobType::Full,
        &user,
        pool,
        s3,
        sqs,
    )
    .await
}

#[post("/depth?<model>", data = "<data>")]
pub async fn submit_depth(
    data: Data<'_>,
    model: Option<String>,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
    sqs: &State<SqsState>,
) -> Result<Json<JobResponse>, ApiError> {
    dispatch_single_image(
        data,
        ConvertOptions { model, ..Default::default() },
        JobType::DepthOnly,
        &user,
        pool,
        s3,
        sqs,
    )
    .await
}

#[get("/<id>")]
pub async fn get_job(
    id: &str,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
) -> Result<Json<JobResponse>, ApiError> {
    let job_id = id
        .parse::<Uuid>()
        .map_err(|_| AppError::Validation("invalid job id".into()))?;

    let job = db::job::find_by_id(pool, job_id)
        .await?
        .ok_or_else(|| AppError::NotFound("job".into()))?;

    if job.user_id != user.user_id {
        return Err(AppError::Forbidden.into());
    }

    let download_url = if job.status == "complete" {
        match &job.output_s3_key {
            Some(key) => storage::presigned_get_url(
                &s3.client,
                &s3.bucket,
                key,
                Duration::from_secs(86400),
            )
            .await
            .ok(),
            None => None,
        }
    } else {
        None
    };

    Ok(Json(job_to_response(job, download_url)))
}

#[get("/list?<limit>&<offset>")]
pub async fn list_jobs(
    limit: Option<i64>,
    offset: Option<i64>,
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Json<JobListResponse>, ApiError> {
    let limit = limit.unwrap_or(20).clamp(1, 100);
    let offset = offset.unwrap_or(0).max(0);

    let jobs = db::job::list_for_user(pool, user.user_id, limit, offset).await?;
    let total = jobs.len();

    let responses = jobs
        .into_iter()
        .map(|j| job_to_response(j, None))
        .collect();

    Ok(Json(JobListResponse {
        jobs: responses,
        total,
    }))
}

#[delete("/<id>")]
pub async fn cancel_job(
    id: &str,
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Status, ApiError> {
    let job_id = id
        .parse::<Uuid>()
        .map_err(|_| AppError::Validation("invalid job id".into()))?;

    let cancelled = db::job::cancel(pool, job_id, user.user_id).await?;
    if cancelled {
        Ok(Status::NoContent)
    } else {
        Err(AppError::NotFound("pending job".into()).into())
    }
}

#[get("/<id>/download")]
pub async fn download_result(
    id: &str,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
) -> Result<rocket::response::Redirect, ApiError> {
    let job_id = id
        .parse::<Uuid>()
        .map_err(|_| AppError::Validation("invalid job id".into()))?;

    let job = db::job::find_by_id(pool, job_id)
        .await?
        .ok_or_else(|| AppError::NotFound("job".into()))?;

    if job.user_id != user.user_id {
        return Err(AppError::Forbidden.into());
    }

    if job.status != "complete" {
        return Err(AppError::Validation("job is not complete".into()).into());
    }

    let key = job
        .output_s3_key
        .ok_or_else(|| AppError::Internal("job complete but missing output key".into()))?;

    let url =
        storage::presigned_get_url(&s3.client, &s3.bucket, &key, Duration::from_secs(86400))
            .await?;

    Ok(rocket::response::Redirect::temporary(url))
}
