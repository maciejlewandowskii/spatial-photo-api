use aws_sdk_s3::presigning::PresigningConfig;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client as S3Client;
use std::time::Duration;

use crate::error::AppError;

pub async fn upload(
    client: &S3Client,
    bucket: &str,
    key: &str,
    data: Vec<u8>,
    content_type: &str,
) -> Result<(), AppError> {
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .body(ByteStream::from(data))
        .content_type(content_type)
        .send()
        .await
        .map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(())
}

pub async fn download(
    client: &S3Client,
    bucket: &str,
    key: &str,
) -> Result<Vec<u8>, AppError> {
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| AppError::Storage(e.to_string()))?;

    resp.body
        .collect()
        .await
        .map(|b| b.into_bytes().to_vec())
        .map_err(|e| AppError::Storage(e.to_string()))
}

/// Generates a presigned GET URL valid for the given duration.
pub async fn presigned_get_url(
    client: &S3Client,
    bucket: &str,
    key: &str,
    expires_in: Duration,
) -> Result<String, AppError> {
    let config = PresigningConfig::expires_in(expires_in)
        .map_err(|e| AppError::Storage(e.to_string()))?;

    let presigned = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .presigned(config)
        .await
        .map_err(|e| AppError::Storage(e.to_string()))?;

    Ok(presigned.uri().to_string())
}

pub async fn delete(client: &S3Client, bucket: &str, key: &str) -> Result<(), AppError> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| AppError::Storage(e.to_string()))?;
    Ok(())
}
