use rocket::serde::json::Json;
use serde::Serialize;
use rocket_okapi::openapi;
use schemars::JsonSchema;
use crate::error::ApiError;
use crate::guards::AuthUser;

#[derive(Serialize, JsonSchema)]
pub struct TokenHistoryResponse {
    pub history: Vec<String>,
}

#[openapi(tag = "Tokens", tag = "Unimplemented")]
#[get("/history")]
pub async fn handler(_user: AuthUser) -> Result<Json<TokenHistoryResponse>, ApiError> {
    // TODO: Implement database fetch for token history
    Ok(Json(TokenHistoryResponse {
        history: vec![],
    }))
}
