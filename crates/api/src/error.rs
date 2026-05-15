use rocket::http::Status;
use rocket::response::Response;
use rocket::serde::json::serde_json;
use shared::error::AppError;
use std::io::Cursor;

use rocket_okapi::response::OpenApiResponderInner;
use rocket_okapi::gen::OpenApiGenerator;
use rocket_okapi::okapi::openapi3::{Responses, RefOr, Response as OpenApiResponse, MediaType};

pub struct HtmxRedirect(pub String);

impl<'r> rocket::response::Responder<'r, 'static> for HtmxRedirect {
    fn respond_to(self, _req: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        Response::build()
            .status(Status::Ok)
            .header(rocket::http::Header::new("HX-Redirect", self.0))
            .ok()
    }
}

#[derive(Debug)]
pub struct ApiError(pub AppError);

impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        Self(err)
    }
}

impl OpenApiResponderInner for ApiError {
    fn responses(gen: &mut OpenApiGenerator) -> rocket_okapi::Result<Responses> {
        let mut responses = Responses::default();
        
        let schema = gen.json_schema::<serde_json::Value>();
        
        responses.responses.insert(
            "400".to_owned(),
            RefOr::Object(OpenApiResponse {
                description: "Bad Request / Validation Error".to_owned(),
                content: {
                    let mut content = rocket_okapi::okapi::Map::new();
                    content.insert("application/json".to_owned(), MediaType {
                        schema: Some(schema.clone()),
                        ..Default::default()
                    });
                    content
                },
                ..Default::default()
            })
        );
        
        responses.responses.insert(
            "401".to_owned(),
            RefOr::Object(OpenApiResponse {
                description: "Unauthorized".to_owned(),
                ..Default::default()
            })
        );

        responses.responses.insert(
            "500".to_owned(),
            RefOr::Object(OpenApiResponse {
                description: "Internal Server Error".to_owned(),
                ..Default::default()
            })
        );

        Ok(responses)
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
            AppError::Validation(_) => Status::BadRequest,
            AppError::Database(_) 
            | AppError::Internal(_)
            | AppError::Storage(_)
            | AppError::Job(_)
            | AppError::Queue(_) => Status::InternalServerError,
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
            .header(rocket::http::ContentType::JSON)
            .sized_body(body.len(), Cursor::new(body))
            .ok()
    }
}
