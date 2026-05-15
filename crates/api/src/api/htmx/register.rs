use rocket::form::Form;
use rocket::State;
use rocket::http::CookieJar;
use shared::db::PgPool;

use crate::error::{ApiError, HtmxRedirect};
use crate::guards::JwtSecret;
use crate::api::auth::register::RegisterRequest;
use crate::api::auth::utils::{validate_email, validate_password, hash_password, issue_auth_response};
use shared::db;

#[post("/register", data = "<body>")]
pub async fn handler(
    body: Form<RegisterRequest>,
    pool: &State<PgPool>,
    jwt: &State<JwtSecret>,
    cookies: &CookieJar<'_>,
) -> Result<HtmxRedirect, ApiError> {
    let req = body.into_inner();
    
    validate_email(&req.email)?;
    validate_password(&req.password)?;
    let hash = hash_password(&req.password)?;
    let user = db::user::create(pool, &req.email.to_lowercase(), &hash).await?;
    let resp = issue_auth_response(pool, user.id, &user.email, &jwt.0).await?;
    
    cookies.add(rocket::http::Cookie::build(("access_token", resp.access_token.clone()))
        .path("/")
        .http_only(true)
        .build());
    
    Ok(HtmxRedirect("/dashboard".to_string()))
}
