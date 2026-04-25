use rocket::serde::json::Json;
use serde::{Deserialize, Serialize};
use validator::Validate;
use shared::jobs::JobType;
use shared::error::AppError;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use crate::error::ApiError;
use crate::guards::AuthUser;

#[derive(Deserialize, Validate, JsonSchema)]
pub struct EstimateRequest {
    pub job_type: JobType,
}

#[derive(Serialize, JsonSchema)]
pub struct EstimateResponse {
    pub estimated_cost: i32,
}

#[openapi(tag = "Tokens")]
#[post("/estimate", data = "<body>")]
pub async fn handler(_user: AuthUser, body: Json<EstimateRequest>) -> Result<Json<EstimateResponse>, ApiError> {
    body.validate().map_err(|e| AppError::Validation(e.to_string()))?;
    let req = body.into_inner();
    let cost = req.job_type.token_cost();
    
    Ok(Json(EstimateResponse {
        estimated_cost: cost,
    }))
}
