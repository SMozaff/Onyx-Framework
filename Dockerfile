# Render's Docker runtime expects the Dockerfile at the repository root.
# Keep this production image aligned with deploy/docker/api-server.Dockerfile.
# The application itself remains the existing Rust/Axum api-server binary.

# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p api-server

FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
EXPOSE 3000 9090
ENV ONYX_BIND=0.0.0.0:3000 ONYX_METRICS_BIND=0.0.0.0:9090 ONYX_ENV=production
CMD ["/usr/local/bin/api-server"]
