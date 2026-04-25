use rocket::Route;

pub mod submit_convert;
pub mod submit_depth;
pub mod submit_compose;
pub mod submit_depth_compose;
pub mod get_job;
pub mod list_jobs;
pub mod cancel_job;
pub mod download_result;
pub mod utils;

pub struct S3State {
    pub client: aws_sdk_s3::Client,
    pub bucket: String,
}

pub struct SqsState {
    pub client: aws_sdk_sqs::Client,
    pub queue_url: String,
}

pub const MAX_IMAGE_BYTES: u64 = 10 * 1024 * 1024;
pub const MAX_DIMENSION: u32 = 8192;

pub fn routes() -> Vec<Route> {
    routes![
        submit_convert::handler,
        submit_depth::handler,
        submit_compose::handler,
        submit_depth_compose::handler,
        get_job::handler,
        list_jobs::handler,
        cancel_job::handler,
        download_result::handler,
    ]
}
