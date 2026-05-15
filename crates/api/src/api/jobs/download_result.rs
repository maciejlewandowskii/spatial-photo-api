use std::time::Duration;
use rocket::State;
use rocket::response::Redirect;
use rocket::http::ContentType;
use shared::db::{self, PgPool};
use shared::error::AppError;
use shared::storage;
use uuid::Uuid;

use crate::error::ApiError;
use crate::guards::AuthUser;
use super::S3State;
use rocket_okapi::openapi;

fn content_type_for_key(key: &str) -> ContentType {
    if key.ends_with(".heic") || key.ends_with(".heif") {
        ContentType::new("image", "heic")
    } else if key.ends_with(".png") {
        ContentType::PNG
    } else if key.ends_with(".jpg") || key.ends_with(".jpeg") {
        ContentType::JPEG
    } else {
        ContentType::Binary
    }
}

fn filename_for_key(key: &str) -> String {
    key.rsplit('/').next().unwrap_or("download").to_string()
}

#[openapi(tag = "Jobs")]
#[get("/<id>/download")]
pub async fn handler(
    id: &str,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
) -> Result<DownloadResponse, ApiError> {
    let job_id = id
        .parse::<Uuid>()
        .map_err(|_| AppError::Validation("invalid job id".into()))?;

    let job = db::job::find_by_id(pool, job_id)
        .await?
        .ok_or_else(|| AppError::NotFound("job".into()))?;

    if job.user_id != user.user_id {
        return Err(AppError::Forbidden.into());
    }

    if job.status != "complete" {
        return Err(AppError::Validation("job is not complete".into()).into());
    }

    let key = job
        .output_s3_key
        .ok_or_else(|| AppError::Internal("job complete but missing output key".into()))?;

    if std::env::var("AWS_ENDPOINT_URL").is_ok() {
        // Local dev: stream file directly to avoid LocalStack presigned URL issues
        let bytes = storage::download(&s3.client, &s3.bucket, &key).await?;
        let ct = content_type_for_key(&key);
        let filename = filename_for_key(&key);
        Ok(DownloadResponse::Bytes { bytes, ct, filename })
    } else {
        let url = storage::presigned_get_url(
            &s3.client,
            &s3.bucket,
            &key,
            Duration::from_secs(86400),
        )
        .await?;
        Ok(DownloadResponse::Redirect(Redirect::temporary(url)))
    }
}

pub enum DownloadResponse {
    Redirect(Redirect),
    Bytes { bytes: Vec<u8>, ct: ContentType, filename: String },
}

impl rocket_okapi::response::OpenApiResponderInner for DownloadResponse {
    fn responses(
        _gen: &mut rocket_okapi::gen::OpenApiGenerator,
    ) -> rocket_okapi::Result<rocket_okapi::okapi::openapi3::Responses> {
        Ok(rocket_okapi::okapi::openapi3::Responses::default())
    }
}

impl<'r> rocket::response::Responder<'r, 'static> for DownloadResponse {
    fn respond_to(self, req: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        match self {
            DownloadResponse::Redirect(r) => r.respond_to(req),
            DownloadResponse::Bytes { bytes, ct, filename } => {
                rocket::Response::build()
                    .header(ct)
                    .raw_header(
                        "Content-Disposition",
                        format!("attachment; filename=\"{filename}\""),
                    )
                    .sized_body(bytes.len(), std::io::Cursor::new(bytes))
                    .ok()
            }
        }
    }
}
