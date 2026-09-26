# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config ca-certificates libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p desktop-shell

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates libgtk-3-0 libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2 \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --create-home onyx
COPY --from=builder /workspace/target/release/desktop-shell /usr/local/bin/desktop-shell
USER 10001
ENTRYPOINT ["/usr/local/bin/desktop-shell"]
