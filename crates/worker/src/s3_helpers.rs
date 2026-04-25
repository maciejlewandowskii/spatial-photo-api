use aws_sdk_s3::Client as S3Client;
use shared::error::AppError;

pub async fn download(client: &S3Client, bucket: &str, key: &str) -> Result<Vec<u8>, AppError> {
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|e| AppError::Storage(format!("S3 get_object '{key}': {e}")))?;

    let body = resp
        .body
        .collect()
        .await
        .map_err(|e| AppError::Storage(format!("S3 body collect '{key}': {e}")))?;

    Ok(body.into_bytes().to_vec())
}

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
        .content_type(content_type)
        .body(data.into())
        .send()
        .await
        .map_err(|e| AppError::Storage(format!("S3 put_object '{key}': {e}")))?;

    Ok(())
}
