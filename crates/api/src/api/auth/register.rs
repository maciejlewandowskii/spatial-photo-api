use rocket::serde::json::Json;
use rocket::State;
use rocket::http::{Cookie, CookieJar};
use shared::db::{self, PgPool};
use serde::Deserialize;
use rocket_okapi::openapi;
use schemars::JsonSchema;

use crate::error::ApiError;
use crate::guards::JwtSecret;
use super::utils::{validate_email, validate_password, hash_password, issue_auth_response, AuthResponse};

#[derive(Deserialize, JsonSchema)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
}

#[openapi(tag = "Auth")]
#[post("/register", data = "<body>")]
pub async fn handler(
    body: Json<RegisterRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
    cookies: &CookieJar<'_>,
) -> Result<Json<AuthResponse>, ApiError> {
    validate_email(&body.email)?;
    validate_password(&body.password)?;
    let hash = hash_password(&body.password)?;
    let user = db::user::create(pool, &body.email.to_lowercase(), &hash).await?;
    let resp = issue_auth_response(pool, user.id, &user.email, &jwt.0).await?;
    
    cookies.add(Cookie::build(("access_token", resp.access_token.clone()))
        .path("/")
        .http_only(true)
        .build());
    
    Ok(Json(resp))
}
