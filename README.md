# Spatial Photo API

A Rust-based API for generating Apple Spatial Photos (HEIC) from standard 2D images.

## Features

- **Depth Estimation**: Uses AI models (DepthPro or Depth-Anything) to estimate depth from a single image.
- **Stereo Synthesis**: Generates a stereo pair using DIBR (Depth-Image-Based Rendering).
- **Neural Inpainting**: Fills holes in warped images using the LaMa model.
- **Spatial HEIF Encoding**: Produces HEIC files compatible with Apple Vision Pro.
- **Authentication**: JWT and API Key based authentication.
- **Job System**: Scalable worker-based processing using AWS SQS and S3.
- **WebSocket Notifications**: Real-time progress updates via AWS API Gateway WebSockets.
- **HTMX Dashboard**: Simple web interface for managing jobs.

## Project Structure

- `crates/api`: Rocket-based REST API and Web UI.
- `crates/worker`: SQS-triggered worker for ML processing.
- `crates/websocket`: Lambda handler for WebSocket connections.
- `crates/shared`: Shared database models, auth logic, and AWS helpers.

## Deployment on AWS

The project is designed to run on AWS Lambda.

### Prerequisites

- AWS Account
- PostgreSQL (RDS or Aurora Serverless v2)
- S3 Buckets (uploads and results)
- SQS FIFO Queue
- DynamoDB Table (for WebSocket connections)
- API Gateway (HTTP and WebSocket)

### Environment Variables

The following variables must be set for the API and Worker Lambdas:

- `DATABASE_URL`: PostgreSQL connection string.
- `JWT_SECRET`: Secret key for JWT signing.
- `S3_BUCKET`: Name of the S3 bucket for storage.
- `SQS_QUEUE_URL`: URL of the SQS FIFO queue.
- `DYNAMODB_TABLE`: Name of the DynamoDB table for connections.
- `WEBSOCKET_API_ENDPOINT`: HTTPS endpoint of the WebSocket API Gateway.
- `DEPTH_MODEL_PATH`: Path to the ONNX depth model (e.g., `/opt/depth_model.onnx`).
- `LAMA_MODEL_PATH`: Path to the ONNX LaMa model (e.g., `/opt/lama_model.onnx`).

### Building and Packaging

Use `cargo lambda` to build and package the binaries for AWS Lambda.

```bash
# Build API
cargo lambda build -p api --release --arm64

# Build Worker
cargo lambda build -p worker --release --arm64
```

The worker requires ONNX Runtime and libheif to be present in the Lambda layer or container image.

## Usage

1. Register or Login via the Dashboard or API.
2. Upload a standard image (JPEG/PNG/WEBP).
3. The API will return a `job_id`.
4. Monitor progress via the Dashboard (HTMX/WS) or `GET /jobs/<job_id>`.
5. Once complete, download the `.heic` spatial photo.

## Technical Details

- **Language**: Rust 1.75+
- **Framework**: Rocket 0.5
- **ML Inference**: `ort` (ONNX Runtime)
- **Image Processing**: `image` crate, `libheif-rs`
- **Database**: `sqlx` (PostgreSQL)
- **Frontend**: HTMX + Tailwind CSS + Tera Templates
