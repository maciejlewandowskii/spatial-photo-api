use rocket::http::Status;
use rocket::State;
use shared::db::{self, PgPool};
use shared::error::AppError;
use uuid::Uuid;

use crate::error::ApiError;
use crate::guards::AuthUser;
use rocket_okapi::openapi;

#[openapi(tag = "Jobs")]
#[delete("/<id>")]
pub async fn handler(
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
