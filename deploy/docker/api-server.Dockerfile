# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p api-server

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl tini && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home onyx
COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
USER 10001
EXPOSE 3000 9090
ENV ONYX_BIND=0.0.0.0:3000 ONYX_METRICS_BIND=0.0.0.0:9090 ONYX_ENV=production
HEALTHCHECK --interval=15s --timeout=3s --retries=4 CMD curl --fail http://127.0.0.1:3000/ready || exit 1
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/api-server"]
