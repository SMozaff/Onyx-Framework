# Clerk + Allfather authentication

## Runtime configuration

The ONYX API accepts Clerk session JWTs and exchanges them for the existing
ONYX access/refresh token pair.

Set these server-side variables:

- CLERK_JWT_KEY — the PEM public key from Clerk API Keys.
- CLERK_ISSUER — the Clerk Frontend API issuer URL for the same instance.
- CLERK_ALLFATHER_USER_ID — the Clerk user ID that is permanently designated
  as the ONYX Allfather identity.

Do not put the Clerk secret key or JWT private key in the client. ONYX only
needs the public verification key for session-token verification.

## First-run bootstrap

1. Create the Allfather identity in Clerk and enable Google as a social
   connection in the Clerk Dashboard.
2. Set CLERK_ALLFATHER_USER_ID to that Clerk user ID.
3. Start ONYX against a completely empty identity store.
4. Sign in with Clerk/Google and obtain the Clerk session token.
5. Send that token as an Authorization bearer token to
   POST /api/admin/bootstrap-clerk.
6. ONYX verifies the Clerk signature and issuer, verifies that the subject is
   exactly CLERK_ALLFATHER_USER_ID, verifies that the local identity store is
   empty, creates the canonical Allfather ONYX Admin identity, and returns a
   normal ONYX access/refresh token pair.
7. Remove any bootstrap-only deployment automation. The endpoint permanently
   closes as soon as the first ONYX identity exists.

The Clerk identity is the authentication authority; ONYX remains the
authorization authority. The Allfather binding is server-side and cannot be
created by changing UI state.

## Subsequent Clerk login

POST /api/auth/clerk with a valid Clerk bearer token exchanges the already
linked Clerk subject for normal ONYX tokens. A Clerk subject that has not been
linked to an ONYX identity is denied.

The existing username/password login remains available during migration so
existing clients are not broken. Native Desktop/Admin and Android UI
integration can therefore be migrated independently after the backend
contract is deployed.
