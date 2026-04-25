use rocket::serde::json::Json;
use rocket::State;
use rocket::http::{Cookie, CookieJar};
use shared::db::{self, PgPool};
use shared::error::AppError;
use serde::Deserialize;

use crate::error::ApiError;
use crate::guards::JwtSecret;
use super::utils::{verify_password, issue_auth_response, AuthResponse};

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[post("/login", data = "<body>")]
pub async fn handler(
    body: Json<LoginRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
    cookies: &CookieJar<'_>,
) -> Result<Json<AuthResponse>, ApiError> {
    let user = db::user::find_by_email(pool, &body.email.to_lowercase())
        .await?
        .ok_or(AppError::InvalidCredentials)?;

    if !verify_password(&body.password, &user.password_hash)? {
        return Err(AppError::InvalidCredentials.into());
    }

    let resp = issue_auth_response(pool, user.id, &user.email, &jwt.0).await?;
    
    cookies.add(Cookie::build(("access_token", resp.access_token.clone()))
        .path("/")
        .http_only(true)
        .build());
    
    Ok(Json(resp))
}
