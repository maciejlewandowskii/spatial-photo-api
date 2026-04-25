#!/bin/bash
set -e
export AWS_ACCESS_KEY_ID=test
export AWS_SECRET_ACCESS_KEY=test
export AWS_DEFAULT_REGION=us-east-1

echo "Initializing LocalStack resources..."

# Create S3 buckets
awslocal s3 mb s3://uploads
awslocal s3 mb s3://results

# Create SQS Standard queue (FIFO is a Pro feature in LocalStack)
awslocal sqs create-queue --queue-name JobQueue

# Create DynamoDB table for WebSocket connections
awslocal dynamodb create-table \
    --table-name WsConnections \
    --attribute-definitions AttributeName=connection_id,AttributeType=S AttributeName=job_id,AttributeType=S \
    --key-schema AttributeName=connection_id,KeyType=HASH \
    --provisioned-throughput ReadCapacityUnits=5,WriteCapacityUnits=5 \
    --global-secondary-indexes "[
        {
            \"IndexName\": \"job_id-index\",
            \"KeySchema\": [{\"AttributeName\":\"job_id\",\"KeyType\":\"HASH\"}],
            \"Projection\": {\"ProjectionType\":\"ALL\"}
        }
    ]"

echo "LocalStack initialization complete!"
