# ONYX Authentication System

All ONYX user classes authenticate with ONYX-managed credentials. All-Father no longer uses Google or Supabase to log in.

## All-Father

- Canonical principal: `__allfather__`
- Canonical username: `allfather`
- Login endpoint: Cloudflare Worker `POST /api/auth/login`
- Password storage: salted PHC Argon2id hash in D1
- Required authority: active principal, active system organization, explicit `ALL_FATHER` role, and `is_admin=1`
- CI inputs: GitHub Actions secrets `ONYX_ALLFATHER_USERNAME` and `ONYX_ALLFATHER_PASSWORD`. These are passed only to the isolated local smoke workflow; they do not automatically update the deployed Worker or D1.

The repeatable `npm run provision:allfather-password` command validates the existing canonical principal, hashes the provided password with the Worker-compatible Argon2id parameters, writes only the hash and a non-sensitive audit event, then verifies the stored hash. Test mode can touch only local D1. Production mode requires explicit `--approve-production __allfather__ --expect-database onyx-free-db` flags and must only be run after owner authorization. No production data is modified by CI.

## Organization administrators and Staff

Admins and Staff continue to use ONYX username/email plus password. Passwords remain salted Argon2id hashes in D1. The Worker revalidates current account, organization, role, tenant, and revocation state for protected requests. Client-provided role/admin flags are never authoritative.

## Retired authentication paths

The Worker and Axum no longer expose an All-Father Supabase exchange route. The Admin Shell no longer contains the Google/OAuth helper or Tauri callback command, and build/deployment configuration no longer injects Supabase authentication settings. Historical D1 migrations and their legacy column are retained for migration compatibility only; the current authentication path does not read or write that mapping.
