use rocket::serde::json::Json;
use serde::Serialize;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use crate::error::ApiError;
use crate::guards::AuthUser;

#[derive(Serialize, JsonSchema)]
pub struct BalanceResponse {
    pub tokens: i32,
}

#[openapi(tag = "Tokens")]
#[get("/balance")]
pub async fn handler(user: AuthUser) -> Result<Json<BalanceResponse>, ApiError> {
    Ok(Json(BalanceResponse {
        tokens: user.tokens,
    }))
}
