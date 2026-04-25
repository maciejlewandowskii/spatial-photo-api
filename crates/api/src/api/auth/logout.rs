use rocket::serde::json::Json;
use rocket::State;
use rocket::http::{Cookie, CookieJar, Status};
use rocket::response::Redirect;
use shared::db::{self, PgPool};
use serde::Deserialize;
use rocket_okapi::openapi;
use schemars::JsonSchema;

use crate::error::ApiError;

#[derive(Deserialize, JsonSchema)]
pub struct LogoutRequest {
    pub refresh_token: String,
}

#[openapi(tag = "Auth")]
#[get("/logout")]
pub async fn logout_get(cookies: &CookieJar<'_>) -> Redirect {
    cookies.remove(Cookie::from("access_token"));
    Redirect::to("/login")
}

#[openapi(tag = "Auth")]
#[post("/logout", data = "<body>")]
pub async fn handler(
    body: Json<LogoutRequest>,
    pool: &State<PgPool>,
    cookies: &CookieJar<'_>,
) -> Result<Status, ApiError> {
    let hash = shared::auth::hash_secret(&body.refresh_token);
    if let Some(rt) = db::refresh_token::find_by_hash(pool, &hash).await? {
        db::refresh_token::delete(pool, rt.id).await?;
    }
    cookies.remove(Cookie::from("access_token"));
    Ok(Status::NoContent)
}
