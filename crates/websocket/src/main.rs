use lambda_runtime::{service_fn, Error as LambdaError, LambdaEvent};
use serde::{Deserialize, Serialize};
use shared::auth::decode_access_token;
use shared::ws;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
struct WsEvent {
    #[serde(rename = "requestContext")]
    request_context: RequestContext,
    body: Option<String>,
    #[serde(rename = "queryStringParameters")]
    query_string_parameters: Option<HashMap<String, String>>,
}

#[derive(Debug, Deserialize)]
struct RequestContext {
    #[serde(rename = "eventType")]
    event_type: String,
    #[serde(rename = "connectionId")]
    connection_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
enum ClientMessage {
    Subscribe { job_id: String },
}

#[derive(Debug, Serialize)]
struct WsResponse {
    #[serde(rename = "statusCode")]
    status_code: u16,
}

struct Config {
    ddb_table: String,
    jwt_secret: String,
}

async fn handler(
    event: LambdaEvent<WsEvent>,
    ddb: &aws_sdk_dynamodb::Client,
    cfg: &Config,
) -> Result<WsResponse, LambdaError> {
    let ctx = &event.payload.request_context;

    match ctx.event_type.as_str() {
        "CONNECT" => {
            let token = event
                .payload
                .query_string_parameters
                .as_ref()
                .and_then(|p| p.get("token"))
                .ok_or("missing token query parameter")?;

            let claims = decode_access_token(token, &cfg.jwt_secret)
                .map_err(|e| format!("auth: {e}"))?;

            let user_id: Uuid = claims.sub.parse().map_err(|_| "invalid sub in token")?;

            ws::store_connection(ddb, &cfg.ddb_table, &ctx.connection_id, user_id).await?;

            tracing::info!(connection_id = %ctx.connection_id, user_id = %user_id, "ws connected");
        }

        "DISCONNECT" => {
            ws::delete_connection(ddb, &cfg.ddb_table, &ctx.connection_id).await?;
            tracing::info!(connection_id = %ctx.connection_id, "ws disconnected");
        }

        "MESSAGE" => {
            let body = event.payload.body.as_deref().unwrap_or("{}");
            match serde_json::from_str::<ClientMessage>(body) {
                Ok(ClientMessage::Subscribe { job_id }) => {
                    let job_uuid: Uuid = job_id.parse().map_err(|_| "invalid job_id")?;
                    ws::bind_job(ddb, &cfg.ddb_table, &ctx.connection_id, job_uuid).await?;
                    tracing::info!(
                        connection_id = %ctx.connection_id,
                        job_id = %job_uuid,
                        "ws subscribed to job"
                    );
                }
                Err(e) => {
                    tracing::warn!(connection_id = %ctx.connection_id, error = %e, "unknown ws message");
                }
            }
        }

        other => {
            tracing::warn!(event_type = %other, "unhandled ws event type");
        }
    }

    Ok(WsResponse { status_code: 200 })
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

    let ddb_table =
        std::env::var("DYNAMODB_TABLE").expect("DYNAMODB_TABLE environment variable must be set");
    let jwt_secret =
        std::env::var("JWT_SECRET").expect("JWT_SECRET environment variable must be set");

    let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
    let ddb = aws_sdk_dynamodb::Client::new(&aws_config);

    let cfg = Config { ddb_table, jwt_secret };

    lambda_runtime::run(service_fn(|event| handler(event, &ddb, &cfg))).await
}
