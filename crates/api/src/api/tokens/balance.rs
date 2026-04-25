use rocket::serde::json::Json;
use serde::Serialize;
use crate::error::ApiError;
use crate::guards::AuthUser;

#[derive(Serialize)]
pub struct BalanceResponse {
    pub tokens: i32,
}

#[get("/balance")]
pub async fn handler(user: AuthUser) -> Result<Json<BalanceResponse>, ApiError> {
    Ok(Json(BalanceResponse {
        tokens: user.tokens,
    }))
}
