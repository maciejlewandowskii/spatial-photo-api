use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use serde::Deserialize;
use serde_json::Value as JsonValue;
use shared::jobs::JobMessage;
use tracing::instrument;

mod ws_push;

#[derive(Debug, Deserialize)]
struct SqsEvent {
    #[serde(rename = "Records")]
    records: Vec<SqsRecord>,
}

#[derive(Debug, Deserialize)]
struct SqsRecord {
    body: String,
}

struct Config {
    db_url: String,
    s3_bucket: String,
    ddb_table: String,
    ws_api_endpoint: String,
    model_path: String,
}

#[instrument(skip_all, fields(job_id))]
async fn process_job(
    msg: JobMessage,
    _cfg: &Config,
) -> Result<(), shared::error::AppError> {
    tracing::Span::current().record("job_id", msg.job_id.to_string());
    // Phases 4–5: depth estimation, stereo synthesis, inpainting, HEIF encoding
    tracing::info!(job_type = ?msg.job_type, "processing job");
    Ok(())
}

async fn handler(event: LambdaEvent<SqsEvent>, cfg: &Config) -> Result<JsonValue, LambdaError> {
    for record in &event.payload.records {
        match serde_json::from_str::<JobMessage>(&record.body) {
            Ok(msg) => {
                if let Err(e) = process_job(msg, cfg).await {
                    tracing::error!(error = %e, "job processing failed");
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

    let cfg = Config {
        db_url: std::env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
        s3_bucket: std::env::var("S3_BUCKET").expect("S3_BUCKET must be set"),
        ddb_table: std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE must be set"),
        ws_api_endpoint: std::env::var("WEBSOCKET_API_ENDPOINT")
            .expect("WEBSOCKET_API_ENDPOINT must be set"),
        model_path: std::env::var("MODEL_PATH").unwrap_or_else(|_| "/opt/model.onnx".into()),
    };

    lambda_runtime::run(service_fn(|event| handler(event, &cfg))).await
}
