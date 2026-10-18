# ONYX Authentication System

All ONYX user classes authenticate using ONYX-managed credentials. All-Father no longer uses Google or Supabase to log in.

## All-Father

- Canonical principal ID: `__allfather__`
- Canonical username: `allfather`
- Login: ONYX username + password
- Password verification: Argon2id hash stored in D1
- CI secrets: `ONYX_ALLFATHER_USERNAME` and `ONYX_ALLFATHER_PASSWORD` are used by GitHub Actions to call `POST /api/auth/login`.
- GitHub Actions secrets are available to the workflow, not the Worker. The D1 account must already be provisioned with a matching Argon2id hash using a trusted owner-controlled process. CI secrets do not automatically provision D1.

The Worker issues an ONYX access/refresh token pair only after verifying that the principal is active, belongs to an active organization, has explicit `ALL_FATHER` role and admin status, and has a valid password hash. Protected requests re-check current authority in D1.

## Organization administrators and Staff

Admins and Staff use ONYX username/email + password. Passwords are stored as salted Argon2id hashes in D1. Admins manage Staff within their own organization. Staff cannot self-register or elevate their own role.

## Endpoints

```
POST /api/auth/login   → ONYX username/email + password → ONYX session
POST /api/auth/refresh → rotate an ONYX refresh token
POST /api/auth/logout  → revoke ONYX session/token
```

There is no All-Father Google/Supabase login or token-exchange endpoint in the Worker.
