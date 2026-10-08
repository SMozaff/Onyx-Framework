# ONYX Render Deployment

This is the non-AWS production deployment path for the native ONYX backend.

## Architecture

```
Client
  |
  v
Cloudflare Worker (optional edge layer)
  |
  | HTTPS
  v
Render Web Service
  onyx-api
  existing deploy/docker/api-server.Dockerfile
  |
  v
Render PostgreSQL
  onyx-postgres

ONYX object storage:
  Hugging Face Storage Bucket (S3-compatible)
```

The ONYX application remains Rust/Axum/Tokio. Render supplies the remote
compute and PostgreSQL infrastructure; it does not replace ONYX domain logic,
authorization, persistence abstractions, or the Hugging Face blob adapter.

## Provisioning

The repository root contains `render.yaml`. Create a Render Blueprint from
that file and deploy it.

Render will provision:

- `onyx-api` as a Docker web service.
- `onyx-postgres` as PostgreSQL 16.
- `DATABASE_URL` and `ONYX_GOVERNANCE_DATABASE_URL` from the same private
  PostgreSQL connection string.
- `/ready` as the service health check.

The API image is built directly from:

```
deploy/docker/api-server.Dockerfile
```

No AWS account, EKS cluster, Kubernetes ingress, or GHCR-to-EKS deployment
step is required for this path.

## Required secrets

Enter these in Render when prompted by the Blueprint:

- `ONYX_AUTHORITY_SIGNING_KEY`
- `ONYX_CORS_ALLOWED_ORIGINS`
- `ONYX_BLOB_STORE_S3_ACCESS_KEY_ID`
- `ONYX_BLOB_STORE_S3_SECRET_ACCESS_KEY`

For the CORS value, use the actual ONYX client origins. Do not use a wildcard
for a production deployment unless that policy is explicitly intended.

The Hugging Face storage endpoint and bucket are non-secret configuration and
are already declared by the Blueprint.

## Cloudflare Worker

After the Render service is deployed, Render provides a public HTTPS hostname
for `onyx-api`.

Set the Cloudflare Worker variable `ONYX_ORIGIN_URL` to that HTTPS origin.
The Worker remains an ingress proxy only; ONYX continues to own
authentication, authorization, and domain policy.

Do not point `ONYX_ORIGIN_URL` at a local address.

## Database migrations

The existing API composition performs the PostgreSQL migrations during state
construction. The Render service therefore does not need a separate local
migration command or a second database migration image for the initial
deployment.

## Verification

After deployment, verify in this order:

1. Render reports the web service healthy.
2. `GET /health` returns process liveness.
3. `GET /ready` confirms PostgreSQL readiness.
4. Authentication and authorization flows work.
5. PostgreSQL writes/reads succeed.
6. Hugging Face object storage operations succeed.
7. The Cloudflare Worker forwards requests to the Render HTTPS origin.
8. No application logs expose database URLs or credentials.

## AWS

The existing AWS Terraform/Helm material remains available as a separate
enterprise deployment target. It is no longer a prerequisite for running ONYX
in production.
