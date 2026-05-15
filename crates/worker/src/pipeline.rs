use image::ImageFormat;
use shared::{
    db,
    error::AppError,
    jobs::{JobMessage, JobStatus, JobType},
};
use std::io::Cursor;
use uuid::Uuid;

use crate::{
    depth::DepthEstimator,
    encode,
    s3_helpers,
    warp::StereoGenerator,
    ws_push::{WsMessage, WsPusher},
};

pub struct AppState {
    pub pool: shared::db::PgPool,
    pub s3: aws_sdk_s3::Client,
    pub s3_bucket: String,
    pub pusher: WsPusher,
    pub depth_estimator: Option<DepthEstimator>,
    pub stereo_generator: Option<StereoGenerator>,
}

impl AppState {
    async fn push_progress(&self, job_id: Uuid, progress: i32) {
        let id_str = job_id.to_string();
        let msg = WsMessage::Progress { job_id: &id_str, progress };
        if let Err(e) = self.pusher.push_to_job(job_id, &msg, &self.pool).await {
            tracing::warn!(error = %e, "progress push failed");
        }
    }

    async fn push_complete(&self, job_id: Uuid, download_url: &str) {
        let id_str = job_id.to_string();
        let msg = WsMessage::Complete { job_id: &id_str, download_url };
        if let Err(e) = self.pusher.push_to_job(job_id, &msg, &self.pool).await {
            tracing::warn!(error = %e, "complete push failed");
        }
    }

    async fn push_error(&self, job_id: Uuid, message: &str) {
        let id_str = job_id.to_string();
        let msg = WsMessage::Error { job_id: &id_str, message };
        if let Err(e) = self.pusher.push_to_job(job_id, &msg, &self.pool).await {
            tracing::warn!(error = %e, "error push failed");
        }
    }
}

pub async fn process(msg: JobMessage, state: &AppState) -> Result<(), AppError> {
    let job_id = msg.job_id;

    db::job::set_status(&state.pool, job_id, &JobStatus::Processing, None).await?;
    state.push_progress(job_id, 0).await;

    match run_pipeline(msg, state).await {
        Ok(output_key) => {
            db::job::set_output(&state.pool, job_id, &output_key).await?;
            db::job::set_status(&state.pool, job_id, &JobStatus::Complete, None).await?;
            state.push_complete(job_id, &output_key).await;
            Ok(())
        }
        Err(e) => {
            let msg_str = e.to_string();
            db::job::set_status(&state.pool, job_id, &JobStatus::Failed, Some(&msg_str)).await?;
            state.push_error(job_id, &msg_str).await;
            Err(e)
        }
    }
}

async fn run_pipeline(msg: JobMessage, state: &AppState) -> Result<String, AppError> {
    let job_id = msg.job_id;

    let job = db::job::find_by_id(&state.pool, job_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("job {job_id}")))?;

    let input_key = job
        .input_s3_key
        .ok_or_else(|| AppError::Internal(format!("job {job_id} missing input_s3_key")))?;

    state.push_progress(job_id, 10).await;
    let image_bytes = s3_helpers::download(&state.s3, &state.s3_bucket, &input_key).await?;
    let image = image::load_from_memory(&image_bytes)
        .map_err(|e| AppError::Internal(format!("decode image: {e}")))?;

    state.push_progress(job_id, 25).await;
    let estimator = state
        .depth_estimator
        .as_ref()
        .ok_or_else(|| AppError::Internal("depth model not loaded".into()))?;
    let depth = estimator.estimate(&image)?;
    state.push_progress(job_id, 50).await;

    let mut depth_png = Vec::new();
    image::DynamicImage::ImageLuma8(depth.gray.clone())
        .write_to(&mut Cursor::new(&mut depth_png), ImageFormat::Png)
        .map_err(|e| AppError::Internal(format!("encode depth PNG: {e}")))?;

    let depth_key = format!("depth/{job_id}.png");
    s3_helpers::upload(&state.s3, &state.s3_bucket, &depth_key, depth_png, "image/png").await?;

    db::job::set_progress(&state.pool, job_id, 50).await?;

    match msg.job_type {
        JobType::DepthOnly => {
            state.push_progress(job_id, 90).await;
            Ok(depth_key)
        }
        JobType::Full | JobType::DepthCompose | JobType::Compose => {
            state.push_progress(job_id, 60).await;
            
            // Default disparity scales with image width so the 3D effect is
            // consistent regardless of resolution (target ~2% of image width).
            let default_disparity = (image.width() / 50).clamp(20, 200);
            let max_disparity = msg.options.max_disparity.unwrap_or(default_disparity) as f32;

            let generator = state
                .stereo_generator
                .as_ref()
                .ok_or_else(|| AppError::Internal("stereo/lama model not loaded".into()))?;
            let (left_img, right_img) = generator.generate_stereo(&image, &depth, max_disparity)?;
            state.push_progress(job_id, 80).await;

            state.push_progress(job_id, 85).await;
            
            let baseline = msg.options.baseline_mm.unwrap_or(0.065);
            
            let fov = msg.options.horizontal_fov.unwrap_or(60.0);

            let heic_bytes = encode::write_spatial_heic(&left_img, &right_img, baseline, fov)?;
            
            let output_key = format!("results/{job_id}.heic");
            s3_helpers::upload(&state.s3, &state.s3_bucket, &output_key, heic_bytes, "image/heic").await?;
            
            state.push_progress(job_id, 100).await;
            Ok(output_key)
        }
    }
}
