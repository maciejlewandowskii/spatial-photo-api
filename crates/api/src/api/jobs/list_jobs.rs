use rocket::serde::json::Json;
use rocket::State;
use shared::db::{self, PgPool};
use serde::Serialize;

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::utils::{job_to_response, JobResponse};

#[derive(Serialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobResponse>,
    pub total: usize,
}

#[get("/list?<limit>&<offset>")]
pub async fn handler(
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
