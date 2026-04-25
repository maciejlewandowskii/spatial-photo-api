use aws_credential_types::Credentials;
use aws_sigv4::http_request::{
    sign, SignableBody, SignableRequest, SigningSettings,
};
use aws_sigv4::sign::v4;
use aws_smithy_runtime_api::client::identity::Identity;
use reqwest::Client as HttpClient;
use serde::Serialize;
use shared::db::PgPool;
use shared::error::AppError;
use shared::ws;
use std::time::SystemTime;
use uuid::Uuid;

// ── Message types ─────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WsMessage<'a> {
    Progress {
        job_id: &'a str,
        progress: i32,
    },
    Complete {
        job_id: &'a str,
        download_url: &'a str,
    },
    Error {
        job_id: &'a str,
        message: &'a str,
    },
}

// ── Push client ───────────────────────────────────────────────────────────────

pub struct WsPusher {
    http: HttpClient,
    credentials: Credentials,
    region: String,
    /// Base endpoint URL, e.g. https://{id}.execute-api.{region}.amazonaws.com/{stage}
    api_endpoint: String,
    ddb: aws_sdk_dynamodb::Client,
    ddb_table: String,
}

impl WsPusher {
    pub fn new(
        http: HttpClient,
        credentials: Credentials,
        region: String,
        api_endpoint: String,
        ddb: aws_sdk_dynamodb::Client,
        ddb_table: String,
    ) -> Self {
        Self { http, credentials, region, api_endpoint, ddb, ddb_table }
    }

    async fn push_one(&self, connection_id: &str, body: &str) -> Result<bool, AppError> {
        let url = format!(
            "{}/@connections/{}",
            self.api_endpoint.trim_end_matches('/'),
            urlencoding::encode(connection_id)
        );

        let body_bytes = body.as_bytes();

        let headers = [("content-type", "application/json")];
        let signable = SignableRequest::new(
            "POST",
            &url,
            headers.iter().map(|(k, v)| (*k, *v)),
            SignableBody::Bytes(body_bytes),
        )
        .map_err(|e| AppError::Internal(format!("build signable request: {e}")))?;

        let identity: Identity = self.credentials.clone().into();
        let params = v4::SigningParams::builder()
            .identity(&identity)
            .region(&self.region)
            .name("execute-api")
            .time(SystemTime::now())
            .settings(SigningSettings::default())
            .build()
            .map_err(|e| AppError::Internal(format!("build signing params: {e}")))?;

        let (signing_instructions, _) = sign(signable, &params.into())
            .map_err(|e| AppError::Internal(format!("sign request: {e}")))?
            .into_parts();

        let mut req = self.http.post(&url).body(body.to_string());
        for (name, value) in signing_instructions.headers() {
            req = req.header(name, value);
        }
        req = req.header("content-type", "application/json");

        let resp = req
            .send()
            .await
            .map_err(|e| AppError::Storage(format!("ws push http: {e}")))?;

        Ok(resp.status().as_u16() != 410)
    }

    /// Sends a message to every connection subscribed to a job.
    /// Stale (410 Gone) connections are removed from DynamoDB.
    pub async fn push_to_job(
        &self,
        job_id: Uuid,
        msg: &WsMessage<'_>,
        pool: &PgPool,
    ) -> Result<(), AppError> {
        let _ = pool; // pool reserved for future user verification
        let connection_ids =
            ws::connections_for_job(&self.ddb, &self.ddb_table, job_id).await?;

        if connection_ids.is_empty() {
            return Ok(());
        }

        let body = serde_json::to_string(msg)
            .map_err(|e| AppError::Internal(format!("serialize ws message: {e}")))?;

        for conn_id in connection_ids {
            match self.push_one(&conn_id, &body).await {
                Ok(false) => {
                    let _ = ws::delete_connection(&self.ddb, &self.ddb_table, &conn_id).await;
                }
                Ok(true) => {}
                Err(e) => {
                    tracing::warn!(connection_id = %conn_id, error = %e, "ws push failed");
                }
            }
        }

        Ok(())
    }
}
