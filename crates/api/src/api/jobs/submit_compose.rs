use rocket::data::Data;
use rocket::serde::json::Json;
use rocket::State;
use shared::db::PgPool;

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::utils::{ConvertOptions, JobResponse};
use super::{S3State, SqsState};
use validator::Validate;
use shared::error::AppError;
use rocket_okapi::openapi;

#[openapi(tag = "Jobs")]
#[post("/compose?<options..>", data = "<_data>")]
pub async fn handler(
    _data: Data<'_>,
    options: ConvertOptions,
    user: AuthUser,
    _pool: &State<PgPool>,
    _s3: &State<S3State>,
    _sqs: &State<SqsState>,
) -> Result<Json<JobResponse>, ApiError> {
    options.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    
    // Multipart implementation would go here
    let _user = user;
    
    Err(shared::error::AppError::Internal("multipart compose not yet implemented".into()).into())
}
