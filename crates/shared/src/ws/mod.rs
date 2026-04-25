use aws_sdk_dynamodb::types::AttributeValue;
use aws_sdk_dynamodb::Client as DdbClient;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

use crate::error::AppError;

// ── Connection store (DynamoDB) ───────────────────────────────────────────────
//
// Table schema (provisioned via CDK/Terraform):
//   PK:  connection_id  (String)
//   GSI: job_id-index   PK=job_id (String)  projection=ALL
//   TTL: ttl            (Number, epoch seconds — 2 h from connect time)

const TTL_SECS: u64 = 7200;

pub async fn store_connection(
    ddb: &DdbClient,
    table: &str,
    connection_id: &str,
    user_id: Uuid,
) -> Result<(), AppError> {
    let ttl = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before Unix epoch")
        .as_secs()
        + TTL_SECS;

    let mut item = HashMap::new();
    item.insert("connection_id".into(), AttributeValue::S(connection_id.into()));
    item.insert("user_id".into(), AttributeValue::S(user_id.to_string()));
    item.insert("ttl".into(), AttributeValue::N(ttl.to_string()));

    ddb.put_item()
        .table_name(table)
        .set_item(Some(item))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("DynamoDB put_item: {e}")))?;

    Ok(())
}

pub async fn bind_job(
    ddb: &DdbClient,
    table: &str,
    connection_id: &str,
    job_id: Uuid,
) -> Result<(), AppError> {
    ddb.update_item()
        .table_name(table)
        .key("connection_id", AttributeValue::S(connection_id.into()))
        .update_expression("SET job_id = :jid")
        .expression_attribute_values(":jid", AttributeValue::S(job_id.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("DynamoDB update_item: {e}")))?;

    Ok(())
}

pub async fn delete_connection(
    ddb: &DdbClient,
    table: &str,
    connection_id: &str,
) -> Result<(), AppError> {
    ddb.delete_item()
        .table_name(table)
        .key("connection_id", AttributeValue::S(connection_id.into()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("DynamoDB delete_item: {e}")))?;

    Ok(())
}

/// Returns all connection IDs currently subscribed to a given job.
pub async fn connections_for_job(
    ddb: &DdbClient,
    table: &str,
    job_id: Uuid,
) -> Result<Vec<String>, AppError> {
    let resp = ddb
        .query()
        .table_name(table)
        .index_name("job_id-index")
        .key_condition_expression("job_id = :jid")
        .expression_attribute_values(":jid", AttributeValue::S(job_id.to_string()))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("DynamoDB query: {e}")))?;

    let ids = resp
        .items()
        .iter()
        .filter_map(|item| {
            item.get("connection_id")
                .and_then(|v| v.as_s().ok())
                .map(|s| s.to_string())
        })
        .collect();

    Ok(ids)
}
