use rocket::data::Data;
use rocket::serde::json::Json;
use rocket::State;
use shared::db::PgPool;
use shared::jobs::JobType;

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::utils::{dispatch_single_image, ConvertOptions, JobResponse};
use super::{S3State, SqsState};
use validator::Validate;
use shared::error::AppError;
use rocket_okapi::openapi;

#[openapi(tag = "Jobs")]
#[post("/depth?<options..>", data = "<data>")]
pub async fn handler(
    data: Data<'_>,
    options: ConvertOptions,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
    sqs: &State<SqsState>,
) -> Result<Json<JobResponse>, ApiError> {
    options.validate().map_err(|e| AppError::Validation(e.to_string()))?;

    let res = dispatch_single_image(
        data,
        options,
        JobType::DepthOnly,
        &user,
        pool,
        s3,
        sqs,
    ).await?;
    
    Ok(Json(res))
}
