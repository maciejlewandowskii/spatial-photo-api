use rocket::form::Form;
use rocket::State;
use rocket::http::CookieJar;
use shared::db::PgPool;
use shared::error::AppError;

use crate::error::{ApiError, HtmxRedirect};
use crate::guards::JwtSecret;
use crate::api::auth::login::LoginRequest;
use crate::api::auth::utils::{verify_password, issue_auth_response};
use shared::db;

#[post("/login", data = "<body>")]
pub async fn handler(
    body: Form<LoginRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
    cookies: &CookieJar<'_>,
) -> Result<HtmxRedirect, ApiError> {
    let req = body.into_inner();
    
    let user = db::user::find_by_email(pool, &req.email.to_lowercase())
        .await?
        .ok_or(AppError::InvalidCredentials)?;

    if !verify_password(&req.password, &user.password_hash)? {
        return Err(AppError::InvalidCredentials.into());
    }

    let resp = issue_auth_response(pool, user.id, &user.email, &jwt.0).await?;
    
    cookies.add(rocket::http::Cookie::build(("access_token", resp.access_token.clone()))
        .path("/")
        .http_only(true)
        .build());
    
    Ok(HtmxRedirect("/dashboard".to_string()))
}
