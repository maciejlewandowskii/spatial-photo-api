use rocket::http::Status;
use rocket::outcome::Outcome;
use rocket::request::{FromRequest, Request};
use shared::auth::{decode_access_token, hash_secret};
use shared::db::{self, PgPool};
use shared::error::AppError;
use uuid::Uuid;

use serde::Serialize;

pub struct JwtSecret(pub String);

#[derive(Debug, Clone, Serialize)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub email: String,
    pub tokens: i32,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = AppError;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, (Status, Self::Error), Status> {
        let secret = match req.rocket().state::<JwtSecret>() {
            Some(s) => &s.0,
            None => {
                return Outcome::Error((
                    Status::InternalServerError,
                    AppError::Internal("missing JWT secret state".into()),
                ))
            }
        };

        let pool = match req.rocket().state::<PgPool>() {
            Some(p) => p,
            None => {
                return Outcome::Error((
                    Status::InternalServerError,
                    AppError::Internal("missing DB pool state".into()),
                ))
            }
        };

        let raw = extract_bearer(req);

        match raw {
            None => Outcome::Error((Status::Unauthorized, AppError::Unauthorized)),
            Some(token) if token.starts_with("sp_") => {
                authenticate_api_key(token, pool).await
            }
            Some(token) => authenticate_jwt(token, secret, pool).await,
        }
    }
}

fn extract_bearer<'r>(req: &'r Request<'_>) -> Option<&'r str> {
    req.headers()
        .get_one("Authorization")
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| req.headers().get_one("X-API-Key"))
        .or_else(|| req.cookies().get("access_token").map(|c| c.value()))
}

async fn authenticate_jwt(
    token: &str,
    secret: &str,
    pool: &PgPool,
) -> Outcome<AuthUser, (Status, AppError), Status> {
    match decode_access_token(token, secret) {
        Ok(claims) => match claims.sub.parse::<Uuid>() {
            Ok(user_id) => {
                match db::user::find_by_id(pool, user_id).await {
                    Ok(Some(user)) => Outcome::Success(AuthUser {
                        user_id: user.id,
                        email: user.email,
                        tokens: user.tokens,
                    }),
                    _ => Outcome::Error((Status::Unauthorized, AppError::Unauthorized)),
                }
            },
            Err(_) => Outcome::Error((Status::Unauthorized, AppError::InvalidToken)),
        },
        Err(e) => Outcome::Error((Status::Unauthorized, e)),
    }
}

async fn authenticate_api_key(
    key: &str,
    pool: &PgPool,
) -> Outcome<AuthUser, (Status, AppError), Status> {
    let hash = hash_secret(key);

    match db::api_key::find_by_hash(pool, &hash).await {
        Err(e) => Outcome::Error((Status::InternalServerError, e)),
        Ok(None) => Outcome::Error((Status::Unauthorized, AppError::Unauthorized)),
        Ok(Some(api_key)) => {
            let _ = db::api_key::touch_last_used(pool, api_key.id).await;

            match db::user::find_by_id(pool, api_key.user_id).await {
                Ok(Some(user)) => Outcome::Success(AuthUser {
                    user_id: user.id,
                    email: user.email,
                    tokens: user.tokens,
                }),
                Ok(None) => Outcome::Error((Status::Unauthorized, AppError::Unauthorized)),
                Err(e) => Outcome::Error((Status::InternalServerError, e)),
            }
        }
    }
}
