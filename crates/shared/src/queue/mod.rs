use aws_sdk_sqs::Client as SqsClient;

use crate::error::AppError;
use crate::jobs::JobMessage;

pub async fn enqueue(
    client: &SqsClient,
    queue_url: &str,
    message: &JobMessage,
) -> Result<(), AppError> {
    let body = serde_json::to_string(message)
        .map_err(|e| AppError::Internal(format!("serialize job message: {e}")))?;

    client
        .send_message()
        .queue_url(queue_url)
        .message_body(body)
        .send()
        .await
        .map_err(|e| AppError::Queue(e.to_string()))?;

    Ok(())
}
