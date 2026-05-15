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

## Development

### 1. Local Setup

Clone the repository and install dependencies:

```bash
# Install Rust dependencies
cargo build

# Install Infrastructure dependencies
cd infra
npm install
```

### 2. Local Infrastructure

The easiest way to set up the required infrastructure (PostgreSQL, S3, SQS, DynamoDB) is using Docker Compose:

```bash
# Start all local services
docker-compose up -d
```

This will start:
- **PostgreSQL**: Available at `localhost:5432`
- **LocalStack**: Emulating S3, SQS, and DynamoDB at `localhost:4566`

A setup script (`scripts/init-localstack.sh`) runs automatically to create the required buckets, queues, and tables.

Create a `.env` file in the project root:

```env
DATABASE_URL=postgres://dev:dev@localhost:5432/spatial_api
JWT_SECRET=your-local-secret
S3_BUCKET=uploads
SQS_QUEUE_URL=http://sqs.us-east-1.localhost.localstack.cloud:4566/000000000000/JobQueue
DYNAMODB_TABLE=WsConnections
WEBSOCKET_API_ENDPOINT=http://localhost:4566
AWS_ACCESS_KEY_ID=test
AWS_SECRET_ACCESS_KEY=test
AWS_REGION=us-east-1
AWS_ENDPOINT_URL=http://localhost:4566
LOCAL_POLL=true
```

*Note: The AWS SDK for Rust automatically respects the `AWS_ENDPOINT_URL` variable to route requests to LocalStack.*

### 3. Running Locally

You can run the API server locally:

```bash
# Run API
cd crates/api
cargo run
```

The worker and websocket components are designed to run as AWS Lambda functions, but can be tested locally using `cargo lambda watch`.

### 4. Code Quality

Always run clippy and format your code before committing:

```bash
# Run Clippy
cargo clippy --workspace -- -D warnings

# Format Code
cargo fmt --all

# Run Tests
cargo test --workspace
```

### 5. API Documentation

When running locally, the OpenAPI documentation is available at:
- Scalar UI: `http://localhost:8000/docs`
