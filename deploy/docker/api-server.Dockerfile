# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p api-server

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime
COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
USER nonroot
EXPOSE 3000 9090
ENV ONYX_BIND=0.0.0.0:3000 ONYX_METRICS_BIND=0.0.0.0:9090 ONYX_ENV=production
ENTRYPOINT ["/usr/local/bin/api-server"]
