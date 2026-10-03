# syntax=docker/dockerfile:1.7
FROM rust:1.98-slim AS builder
WORKDIR /workspace
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p api-server

# gcr.io/distroless/cc-debian12 ships glibc + libssl + ca-certs and nothing else,
# eliminating the util-linux, acl, systemd, ncurses, perl, and curl CVE surface
# present in debian:bookworm-slim. The nonroot variant enforces UID 65532.
# HEALTHCHECK is omitted: liveness/readiness probes on /ready are handled by the
# orchestrator, which also removes the runtime curl dependency entirely.
FROM gcr.io/distroless/cc-debian12:nonroot AS runtime
COPY --from=builder /workspace/target/release/api-server /usr/local/bin/api-server
EXPOSE 3000 9090
ENV ONYX_BIND=0.0.0.0:3000 ONYX_METRICS_BIND=0.0.0.0:9090 ONYX_ENV=production
CMD ["/usr/local/bin/api-server"]
