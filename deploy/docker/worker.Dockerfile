# syntax=docker/dockerfile:1.7
FROM rust:1.97-slim AS builder
WORKDIR /workspace
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
COPY . .
RUN cargo build --locked --release -p worker

FROM gcr.io/distroless/cc-debian13:nonroot AS runtime
COPY --from=builder /workspace/target/release/worker /usr/local/bin/worker
CMD ["/usr/local/bin/worker"]
