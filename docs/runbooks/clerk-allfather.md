# ONYX Clerk / Allfather identity model

## Authority model

- Clerk authenticates the human identity, including Google sign-in.
- ONYX is the authoritative account registry and authorization system.
- A Clerk identity does **not** self-register an ONYX account.
- The configured Clerk user id in `CLERK_ALLFATHER_USER_ID` is the permanent Allfather identity.
- On startup, when Clerk is configured, ONYX ensures the Allfather principal exists as an active administrator. This is configuration-driven provisioning, not a bootstrap endpoint.
- There is no `/api/admin/bootstrap` or Clerk bootstrap endpoint.

## Production configuration

Set these server-side secrets/variables:

- `CLERK_ISSUER` — exact Clerk JWT issuer.
- `CLERK_ALLFATHER_USER_ID` — Clerk subject permanently bound to Allfather.
- `CLERK_JWKS_URL` — Clerk JWKS endpoint; optional when the issuer's standard `/.well-known/jwks.json` endpoint is used.

Never expose these as client secrets. The Clerk publishable key belongs in client configuration; the server verifies the resulting Clerk session token.

## Authentication flow

1. User signs in with Clerk/Google.
2. Client sends the Clerk session JWT to `POST /api/auth/clerk`.
3. API verifies RS256 signature, issuer, expiration and not-before claims against Clerk JWKS.
4. Allfather's configured Clerk subject maps to the canonical `allfather` ONYX principal.
5. Other Clerk subjects map to their pre-provisioned ONYX identity (`clerk:<subject>`).
6. If no ONYX identity exists, the API returns `403 ONYX_ACCOUNT_NOT_PROVISIONED`; it does not create one.
7. ONYX issues its normal access/refresh session tokens and applies its existing authorization model.

## Provisioning other users

Allfather uses the authenticated admin surface:

`POST /api/admin/clerk-users`

with a Clerk user id and the desired ONYX organization/class/authority attributes. The endpoint is ONYX-admin protected. The provisioned identity is then eligible to exchange its Clerk session for an ONYX session.

The provisioning endpoint creates an unusable local password credential because Clerk is the authentication authority for these identities; it is not a second login path.

## Security boundary

A valid Clerk token answers **who the human is**. It does not grant ONYX access by itself. ONYX account registration, activation, organization membership, class, admin authority, and all command authorization remain server-side ONYX decisions.
