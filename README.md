# ONYX GitHub Actions workflows — reviewed package

Replace the corresponding files under `.github/workflows/` with these files.

## Required GitHub Actions secret before deploying

The Cloudflare deployment workflow now rejects wildcard CORS and requires `ONYX_CORS_ORIGINS`.

Create a repository Actions secret named `ONYX_CORS_ORIGINS` containing the exact trusted browser origins for the deployed ONYX web/admin interfaces, comma-separated, with no wildcard. Use only origins you control; do not guess the production frontend domains.

`ONYX_JWT_SECRET` must also be configured before the first Worker deployment. Keep it stable across deployments. The workflow preserves an already-configured Worker secret and will not silently generate an untracked replacement.

## Scope of changes

- `deploy-cloudflare-edge.yml`: removes wildcard CORS, requires the explicit CORS allowlist, and fails closed if the JWT secret is missing during first-time initialization.
- The other nine workflow files are included unchanged because the supplied archive alone does not establish enough evidence to safely alter their behavior.

## Validation limitation

The workflow YAML files were parsed successfully. This package has not been executed by GitHub Actions, and it does not prove that a live deployment or authentication flow succeeds. After replacing the files and adding the required secret, run the Cloudflare Worker CI and deployment workflows in GitHub Actions.
