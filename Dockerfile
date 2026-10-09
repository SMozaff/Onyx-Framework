# Production container image for the existing Rust/Axum ONYX API.
# Cloudflare Workers Containers builds this image and runs the api-server binary
# behind the ONYX Cloudflare Worker.
#
# Build and runtime both use Debian 12 so the binary does not require a newer
# glibc than the distroless runtime provides.

# syntax=docker/dockerfile:1.7
FROM rust:1.97-bookworm AS builder
WORKDIR /workspace
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p api-server

FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
EXPOSE 10000 9090
ENV ONYX_BIND=0.0.0.0:10000 ONYX_METRICS_BIND=127.0.0.1:9090 ONYX_ENV=production
CMD ["/usr/local/bin/api-server"]
