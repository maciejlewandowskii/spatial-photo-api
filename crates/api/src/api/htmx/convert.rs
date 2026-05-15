use rocket::form::Form;
use rocket::fs::TempFile;
use rocket::State;
use shared::db::PgPool;
use shared::jobs::JobType;
use rocket_dyn_templates::{Template, context};

use crate::error::ApiError;
use crate::guards::AuthUser;
use crate::api::jobs::utils::{dispatch_single_image_from_bytes, ConvertOptions};
use crate::api::jobs::{S3State, SqsState};

#[derive(FromForm)]
pub struct ConvertForm<'r> {
    pub image: TempFile<'r>,
    pub max_disparity: Option<u32>,
    pub model: Option<String>,
    pub horizontal_fov: Option<f32>,
    pub baseline_mm: Option<f32>,
}

#[post("/convert", data = "<form>")]
pub async fn handler(
    mut form: Form<ConvertForm<'_>>,
    user: AuthUser,
    pool: &State<PgPool>,
    s3: &State<S3State>,
    sqs: &State<SqsState>,
) -> Result<Template, ApiError> {
    let options = ConvertOptions {
        max_disparity: form.max_disparity,
        model: form.model.clone(),
        horizontal_fov: form.horizontal_fov,
        baseline_mm: form.baseline_mm,
    };

    let bytes = match form.image.path() {
        Some(path) => {
            tracing::info!(path = %path.display(), "reading temp file from disk");
            tokio::fs::read(path)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "failed to read temp file from disk");
                    shared::error::AppError::Internal(format!("read temp file: {e}"))
                })?
        },
        None => {
            tracing::info!("reading temp file from memory via persist");
            let mut temp_path = std::env::temp_dir();
            temp_path.push(uuid::Uuid::new_v4().to_string());
            
            form.image.persist_to(&temp_path)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, path = %temp_path.display(), "failed to persist temp file");
                    shared::error::AppError::Internal(format!("persist temp file: {e}"))
                })?;
                
            let b = tokio::fs::read(&temp_path)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "failed to read persisted temp file");
                    shared::error::AppError::Internal(format!("read persisted file: {e}"))
                })?;
                
            let _ = tokio::fs::remove_file(&temp_path).await;
            b
        }
    };

    tracing::info!(len = bytes.len(), "dispatching job");
    let res = dispatch_single_image_from_bytes(
        bytes,
        options,
        JobType::Full,
        &user,
        pool,
        s3,
        sqs,
    ).await.map_err(|e| {
        tracing::error!(error = ?e, "dispatch_single_image_from_bytes failed");
        e
    })?;
    
    tracing::info!(job_id = %res.id, "job dispatched successfully");
    
    Ok(Template::render("job_list_partial", context! { 
        jobs: vec![res] 
    }))
}
