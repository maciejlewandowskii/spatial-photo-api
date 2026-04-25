use rocket::serde::json::Json;
use rocket::State;
use shared::db::{self, PgPool};
use shared::error::AppError;
use serde::Serialize;
use rocket_okapi::openapi;
use schemars::JsonSchema;

use crate::error::ApiError;
use crate::guards::AuthUser;

#[derive(Serialize, JsonSchema)]
pub struct UserResponse {
    pub id: String,
    pub email: String,
    pub tokens: i32,
    pub created_at: i64,
}

#[openapi(tag = "Auth")]
#[get("/me")]
pub async fn handler(
    user: AuthUser,
    pool: &State<PgPool>,
) -> Result<Json<UserResponse>, ApiError> {
    let u = db::user::find_by_id(pool, user.user_id)
        .await?
        .ok_or_else(|| AppError::NotFound("user".into()))?;

    Ok(Json(UserResponse {
        id: u.id.to_string(),
        email: u.email,
        tokens: u.tokens,
        created_at: u.created_at.unix_timestamp(),
    }))
}
