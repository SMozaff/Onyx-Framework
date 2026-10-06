# ONYX Backend Architecture

## Runtime decision

ONYX remains a native Rust/Axum/Tokio application. The existing API server and worker contain
domain execution, authorization, PostgreSQL transactions, audit/security policy, and background
processing semantics that are not moved into Cloudflare Workers.

Cloudflare is the preferred Tier 1 edge boundary. The repository therefore provides a small
Cloudflare Worker under `deploy/cloudflare/edge-gateway` that proxies the public API route to the
native ONYX origin.

## Trust boundaries

### Tier 1 — Cloudflare edge

Responsibilities:
- public ingress and TLS/network boundary;
- route matching and edge-level abuse controls when configured;
- forwarding HTTP and WebSocket requests to the ONYX origin.

Non-responsibilities:
- no ONYX authorization decisions;
- no PostgreSQL access;
- no object-store credentials;
- no domain/workflow execution.

The edge Worker deliberately forwards the request's authentication material to the native
API. Authorization remains server-side in ONYX.

### Tier 2 — ONYX application

The Rust/Axum/Tokio runtime owns:
- authentication and token revocation;
- capability and command authorization;
- domain validation and workflow semantics;
- PostgreSQL transactions and migrations;
- idempotency, audit, and security policy;
- API response/error semantics;
- background jobs, outbox relay, scheduler, leases, retries, and dead-letter handling.

Cloudflare Workers are not a replacement runtime for this tier.

### Tier 3 — managed persistence/infrastructure

PostgreSQL remains provider-neutral through the existing SQLx/persistence abstractions.
The background worker retains the existing PostgreSQL-backed queue and outbox semantics.

Blob storage is selected through the existing `query_application::BlobStore` port:
- local filesystem for development/test;
- Hugging Face Storage Buckets through their S3-compatible gateway for production.

Provider credentials are server-side environment configuration only.

## Hugging Face object storage

The Hugging Face adapter is isolated in
`crates/infrastructure/huggingface-blob-storage`. It implements the existing
`BlobStore` contract using AWS Signature Version 4 against the Hugging Face S3 gateway.

The configured ONYX production bucket is:

```text
namespace: Arronthemalkavian
bucket: onyx
endpoint: https://s3.hf.co/Arronthemalkavian
region: us-east-1
addressing: path
```

Production configuration:

```text
ONYX_BLOB_STORE_BACKEND=huggingface
ONYX_BLOB_STORE_S3_ENDPOINT=https://s3.hf.co/Arronthemalkavian
ONYX_BLOB_STORE_S3_BUCKET=onyx
ONYX_BLOB_STORE_S3_ACCESS_KEY_ID=HFAK...
ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY=<secret>
```

The access key and secret are Kubernetes Secret values and are never committed to Git.
Hugging Face documents the namespace-scoped endpoint, `us-east-1` region, and path-style
addressing as the required S3 client configuration.

The adapter does not expose S3/Hugging Face types to application or domain crates.

## Background execution decision

The existing ONYX worker remains authoritative for:
- job claiming and leases;
- retry/backoff and attempt accounting;
- deduplication;
- dead-lettering;
- outbox relay;
- five-second scheduler cadence;
- snapshot scheduling.

Cloudflare Queues/Cron are intentionally not substituted for these mechanisms because doing so
would change established transactional and retry semantics without a demonstrated requirement.

## Deployment boundary

The native API/worker deployment remains deployable through the existing container/Helm path.
The Cloudflare Worker is independently deployable through Wrangler and is configured with an
origin URL at deployment time.

This separation keeps Cloudflare-specific configuration out of ONYX domain/application code and
allows the native runtime to remain portable across PostgreSQL providers and deployment targets.
