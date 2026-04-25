use std::time::Duration;
use rocket::State;
use rocket::response::Redirect;
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::storage;
use uuid::Uuid;

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::S3State;
use rocket_okapi::openapi;

#[openapi(tag = "Jobs")]
#[get("/<id>/download")]
pub async fn handler(
    id: &str,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
) -> Result<Redirect, ApiError> {
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

    Ok(Redirect::temporary(url))
}
