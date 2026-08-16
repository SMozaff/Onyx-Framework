# ONYX Increment 7

Increment 7 adds production observability, durable background processing,
Ed25519 authority verification, rate governance, rotating environment secrets,
and SHA-256 audit integrity.

## Production startup

```bash
export ONYX_ENV=production
export DATABASE_URL='postgres://...'
export ONYX_GOVERNANCE_DATABASE_URL='postgres://...'
export ONYX_AUTHORITY_SIGNING_KEY='hex:<64 hex characters>'
# Optional rotation pair; expiry is mandatory when previous is set:
# export ONYX_AUTHORITY_SIGNING_KEY_PREVIOUS='hex:<64 hex characters>'
# export ONYX_AUTHORITY_SIGNING_KEY_PREVIOUS_VALID_UNTIL_UNIX='<unix-seconds>'
export OTEL_EXPORTER_OTLP_ENDPOINT='http://jaeger-collector:4317'
export ONYX_METRICS_BIND='0.0.0.0:9090'

cargo run -p migration-tool -- postgres
cargo run -p worker
```

The Team 6 API retains its SQLite operational store by default, but production
security governance requires PostgreSQL:

```bash
export DATABASE_URL='sqlite://onyx.db?mode=rwc'
export ONYX_GOVERNANCE_DATABASE_URL='postgres://...'
cargo run -p api-server
```

## Verification

```bash
cargo check --workspace
cargo test -p team7-integration-tests --test integration
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
```

Prometheus is available at `http://<ONYX_METRICS_BIND>/metrics`.
