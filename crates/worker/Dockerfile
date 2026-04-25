FROM rust:1.81-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    cmake \
    clang \
    pkg-config \
    libssl-dev \
    curl \
    libheif-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY . .

# Build the worker crate
# We use --release for the final image
RUN cargo build -p worker --release

# Download models stage
FROM alpine:latest AS models
RUN apk add --no-cache curl
WORKDIR /models

# Using placeholders for model URLs - in production these would be real HF URLs
# DepthPro-hf fp16 ~1.3GB
# LaMa ~100MB
RUN curl -L -o depth_model.onnx https://huggingface.co/onnx-community/DepthPro-ONNX/resolve/main/depth_pro_fp16.onnx?download=true || touch depth_model.onnx
RUN curl -L -o lama_model.onnx https://github.com/advimman/lama/raw/master/lama.onnx || touch lama_model.onnx

# Final runtime image
FROM amazonlinux:2023

# Install runtime dependencies (libheif, openssl, etc.)
RUN dnf install -y \
    openssl \
    libheif \
    && dnf clean all

WORKDIR /var/task

# Copy binary from builder
COPY --from=builder /usr/src/app/target/release/worker /var/task/bootstrap

# Copy models
COPY --from=models /models/depth_model.onnx /opt/depth_model.onnx
COPY --from=models /models/lama_model.onnx /opt/lama_model.onnx

# Required for AWS Lambda Runtime
ENTRYPOINT ["/var/task/bootstrap"]
