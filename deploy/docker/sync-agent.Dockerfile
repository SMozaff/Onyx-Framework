# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p sync-agent

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home onyx
COPY --from=builder /workspace/target/release/sync-agent /usr/local/bin/sync-agent
USER 10001
EXPOSE 9090
ENV ONYX_METRICS_BIND=0.0.0.0:9090 ONYX_ENV=production
ENTRYPOINT ["/usr/bin/tini","--"]
CMD ["/usr/local/bin/sync-agent"]
