use rocket::http::{ContentType, Status};
use rocket::response::Response;
use rocket::serde::json::serde_json;
use shared::error::AppError;
use std::io::Cursor;

#[derive(Debug)]
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(e: AppError) -> Self {
        ApiError(e)
    }
}

impl<'r> rocket::response::Responder<'r, 'static> for ApiError {
    fn respond_to(self, _req: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        let status = match &self.0 {
            AppError::InvalidCredentials
            | AppError::Unauthorized
            | AppError::InvalidToken
            | AppError::TokenExpired => Status::Unauthorized,
            AppError::Forbidden => Status::Forbidden,
            AppError::NotFound(_) => Status::NotFound,
            AppError::InsufficientTokens => Status::PaymentRequired,
            AppError::Validation(_) => Status::UnprocessableEntity,
            _ => Status::InternalServerError,
        };

        let message = if status.code >= 500 {
            tracing::error!(error = %self.0, "internal error");
            "internal server error".to_string()
        } else {
            self.0.to_string()
        };

        let body = serde_json::json!({ "error": message }).to_string();

        Response::build()
            .status(status)
            .header(ContentType::JSON)
            .sized_body(body.len(), Cursor::new(body))
            .ok()
    }
}
