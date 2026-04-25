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
- `crates/worker`: SQS-triggered worker for ML processing (Docker-based).
- `crates/websocket`: Lambda handler for WebSocket connections.
- `crates/shared`: Shared database models, auth logic, and AWS helpers.
- `infra`: AWS CDK (TypeScript) for Infrastructure as Code.

## Infrastructure (IaC)

The project includes a complete AWS CDK setup in the `infra` directory:

- **DatabaseStack**: Aurora Serverless v2 (PostgreSQL) + VPC + RDS Proxy.
- **ApiStack**: HTTP API, WebSocket API, S3 Buckets, SQS Queue, DynamoDB Table, and API/WebSocket Lambdas.
- **WorkerStack**: Docker-based Lambda Worker integrated with SQS.

## Deployment

### 1. Prerequisites

- [Rust](https://www.rust-lang.org/) and [Cargo Lambda](https://cargo-lambda.info/)
- [Node.js](https://nodejs.org/) and `npm`
- [Docker](https://www.docker.com/) (required for building the Worker image)
- [AWS CLI](https://aws.amazon.com/cli/) configured with your credentials

### 2. Build Rust Binaries

Build the API and WebSocket lambdas for the ARM64 architecture:

```bash
cargo lambda build -p api --release --arm64
cargo lambda build -p websocket --release --arm64
```

*Note: The Worker is built automatically by CDK using the provided Dockerfile.*

### 3. Deploy Infrastructure

Navigate to the `infra` directory and deploy using CDK:

```bash
cd infra
npm install
npm run deploy
```

## Configuration

The following environment variables are managed by the CDK stacks but can be customized in `infra/lib/api-stack.ts`:

- `DATABASE_URL`: Managed via RDS Secret.
- `JWT_SECRET`: Secret key for JWT signing.
- `S3_BUCKET`: Auto-generated bucket name.
- `SQS_QUEUE_URL`: Auto-generated queue URL.
- `DYNAMODB_TABLE`: Auto-generated table name.
- `WEBSOCKET_API_ENDPOINT`: Auto-generated WebSocket endpoint.

## Usage

1. **Access the UI**: Once deployed, the CDK output will provide the HTTP API URL. Open it in your browser to access the HTMX dashboard.
2. **Register/Login**: Create an account to receive your initial token balance (1000 tokens).
3. **Upload**: Select a 2D image (JPG/PNG/WEBP).
4. **Monitor**: Watch the real-time progress via WebSockets on the dashboard.
5. **Download**: Once the status is "complete", click download to get your Apple Spatial Photo (.heic).

## Technical Details

- **Language**: Rust 1.81+
- **Framework**: Rocket 0.5
- **Infrastructure**: AWS CDK (TypeScript)
- **ML Inference**: `ort` (ONNX Runtime) with DepthPro and LaMa models.
- **Image Processing**: `image` crate and `libheif-rs`.
- **Database**: `sqlx` with PostgreSQL on Aurora Serverless v2.
- **Frontend**: HTMX, Tailwind CSS, and Tera Templates.
