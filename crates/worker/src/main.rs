use aws_credential_types::provider::ProvideCredentials;
use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use shared::jobs::JobMessage;

mod depth;
mod encode;
mod pipeline;
mod s3_helpers;
mod warp;
mod ws_push;

use pipeline::AppState;
use ws_push::WsPusher;

#[derive(Debug, Deserialize)]
struct SqsEvent {
    #[serde(rename = "Records")]
    records: Vec<SqsRecord>,
}

#[derive(Debug, Deserialize)]
struct SqsRecord {
    body: String,
}

async fn handler(event: LambdaEvent<SqsEvent>, state: &AppState) -> Result<JsonValue, LambdaError> {
    for record in &event.payload.records {
        match serde_json::from_str::<JobMessage>(&record.body) {
            Ok(msg) => {
                let job_id = msg.job_id;
                if let Err(e) = pipeline::process(msg, state).await {
                    tracing::error!(job_id = %job_id, error = %e, "job processing failed");
                }
            }
            Err(e) => {
                tracing::error!(body = %record.body, error = %e, "failed to parse SQS record");
            }
        }
    }
    Ok(serde_json::json!({"batchItemFailures": []}))
}

#[tokio::main]
async fn main() -> Result<(), LambdaError> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .json()
        .init();

    let db_url = std::env::var("DATABASE_URL").map_err(|_| LambdaError::from("DATABASE_URL must be set"))?;
    let s3_bucket = std::env::var("S3_BUCKET").map_err(|_| LambdaError::from("S3_BUCKET must be set"))?;
    let ddb_table = std::env::var("DYNAMODB_TABLE").map_err(|_| LambdaError::from("DYNAMODB_TABLE must be set"))?;
    let ws_api_endpoint =
        std::env::var("WEBSOCKET_API_ENDPOINT").map_err(|_| LambdaError::from("WEBSOCKET_API_ENDPOINT must be set"))?;
    let depth_model_path =
        std::env::var("DEPTH_MODEL_PATH").unwrap_or_else(|_| "/opt/depth_model.onnx".into());
    let lama_model_path =
        std::env::var("LAMA_MODEL_PATH").unwrap_or_else(|_| "/opt/lama_model.onnx".into());
    let region = std::env::var("AWS_REGION").unwrap_or_else(|_| "us-east-1".into());

    let pool = shared::db::PgPool::connect(&db_url)
        .await
        .map_err(|e| LambdaError::from(format!("failed to connect to database: {e}")))?;

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;

    let credentials = aws_config
        .credentials_provider()
        .ok_or_else(|| LambdaError::from("no credentials provider in aws config"))?
        .provide_credentials()
        .await
        .map_err(|e| LambdaError::from(format!("failed to resolve AWS credentials: {e}")))?;

    let s3 = aws_sdk_s3::Client::new(&aws_config);
    let sqs = aws_sdk_sqs::Client::new(&aws_config);
    let ddb = aws_sdk_dynamodb::Client::new(&aws_config);

    let pusher = WsPusher::new(
        HttpClient::new(),
        credentials,
        region,
        ws_api_endpoint,
        ddb.clone(),
        ddb_table.clone(),
    );

    let depth_estimator =
        depth::DepthEstimator::load(&depth_model_path)
            .map_err(|e| LambdaError::from(format!("failed to load depth model: {e}")))?;

    let stereo_generator =
        warp::StereoGenerator::load(&lama_model_path)
            .map_err(|e| LambdaError::from(format!("failed to load lama model: {e}")))?;

    let state = AppState {
        pool,
        s3,
        s3_bucket,
        pusher,
        depth_estimator,
        stereo_generator,
    };

    // SQS client kept for potential future use (dead-letter, visibility timeout extension)
    let _ = sqs;

    lambda_runtime::run(service_fn(|event| handler(event, &state))).await
}
