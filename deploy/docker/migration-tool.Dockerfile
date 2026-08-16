# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo generate-lockfile && cargo build --locked --release -p migration-tool

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates tini && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home onyx
COPY --from=builder /workspace/target/release/migration-tool /usr/local/bin/migration-tool
USER 10001
ENTRYPOINT ["/usr/bin/tini","--","/usr/local/bin/migration-tool"]
CMD ["status"]
