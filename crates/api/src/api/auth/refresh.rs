use rocket::serde::json::Json;
use rocket::State;
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::auth::encode_access_token;
use serde::{Deserialize, Serialize};
use rocket_okapi::openapi;
use schemars::JsonSchema;

use crate::error::ApiError;
use crate::guards::JwtSecret;

#[derive(Deserialize, JsonSchema)]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Serialize, JsonSchema)]
pub struct AccessTokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
}

#[openapi(tag = "Auth")]
#[post("/refresh", data = "<body>")]
pub async fn handler(
    body: Json<RefreshRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
) -> Result<Json<AccessTokenResponse>, ApiError> {
    let hash = shared::auth::hash_secret(&body.refresh_token);
    let rt = db::refresh_token::find_by_hash(pool, &hash)
        .await?
        .ok_or(AppError::InvalidToken)?;

    let user = db::user::find_by_id(pool, rt.user_id)
        .await?
        .ok_or(AppError::NotFound("user".into()))?;

    db::refresh_token::delete(pool, rt.id).await?;

    let access_token = encode_access_token(user.id, &user.email, &jwt.0)?;
    Ok(Json(AccessTokenResponse {
        access_token,
        token_type: "Bearer",
        expires_in: shared::auth::ACCESS_TOKEN_TTL_SECS,
    }))
}
