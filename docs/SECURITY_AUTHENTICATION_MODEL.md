# ONYX Authentication and Authorization Model

**Status: authoritative security requirement for all ONYX clients and deployments.**

This document governs the ONYX authentication migration. Do not replace this design with a single third-party identity provider for every user class.

## Identity and authority boundaries

### All-Father: Clerk + Google only

- The designated All-Father signs in using the Google identity `so.muzaff@gmail.com`.
- Clerk authenticates that Google identity.
- The backend verifies the Clerk token's signature, issuer, expiry, and subject, then checks the subject's verified primary email with Clerk.
- Only that verified identity may receive the `ALL_FATHER` ONYX role.
- The All-Father does not sign in with an ONYX username/password.
- The All-Father provisions organizations and organization administrators.

### Organization administrators: ONYX-managed credentials

- Admins authenticate using either username + password or email + password.
- Credentials and password hashes are managed by ONYX; do not require Admins to sign in through Clerk or Supabase Auth.
- Passwords must be stored as strong salted Argon2id hashes. Never store plaintext passwords or placeholder hashes.
- D1 is authoritative for user identity, active status, role, and organization membership in the Cloudflare deployment.

### Staff: admin-provisioned ONYX accounts

- Staff accounts are created, deactivated, and managed by an authorized Admin.
- Staff cannot self-register, create organizations, create Admins, or elevate their own role.
- An Admin may manage Staff only within that Admin's own organization.
- Staff use ONYX username/email + password authentication, not Clerk or Supabase Auth.

## Roles and authorization

At minimum, distinguish:

- `ALL_FATHER`: cross-organization authority limited to explicitly defined All-Father operations, including organization and Admin provisioning.
- `ORGANIZATION_ADMIN`: authority within the Admin's own active organization, including Staff provisioning/deactivation.
- `STAFF`: only the capabilities granted to Staff within their own organization.

Every protected operation must enforce role and organization boundaries server-side. Never trust role, organization ID, or account status supplied by a client. Re-fetch or validate authoritative account status when exchanging/refreshing sessions and reject deactivated accounts. The UI is not an authorization boundary.

## Cloudflare Worker authentication endpoints

- `POST /api/auth/clerk`: accept a Clerk bearer token, verify the Clerk identity, enforce the designated verified All-Father email, map to the provisioned ONYX All-Father account in D1, and issue an ONYX session.
- `POST /api/auth/login`: accept `username` or `email` plus password; find the ONYX account in D1, verify its Argon2id password hash, reject inactive accounts and the All-Father account, then issue an ONYX session.
- `POST /api/auth/refresh`: validate and rotate an ONYX refresh token; reject revoked tokens and inactive accounts.
- `POST /api/auth/logout`: revoke the presented ONYX session/refresh token.

Clerk is used only for the All-Father identity. Supabase Auth is not the password authority for Admins or Staff. Supabase may be used for unrelated services only if it does not override this model.

## Authoritative data and safe migration

D1 owns Cloudflare-deployment records for ONYX users, roles, active state, organization membership, and authorization decisions. Password hashes are stored only in the ONYX user record. Keep session signing keys and Clerk secrets in deployment secrets, never in source control.

Do not switch production traffic until the Worker implementation and migrations satisfy this contract and GitHub Actions verifies the relevant build and authorization tests. During migration, do not silently fall back to another identity provider or grant broad access when configuration is missing.
