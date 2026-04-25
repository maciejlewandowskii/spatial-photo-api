use std::time::Duration;
use rocket::serde::json::Json;
use rocket::State;
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::storage;
use uuid::Uuid;

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::utils::{job_to_response, JobResponse};
use super::S3State;
use rocket_okapi::openapi;

#[openapi(tag = "Jobs")]
#[get("/<id>")]
pub async fn handler(
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
