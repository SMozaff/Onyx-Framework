# Desktop token storage policy (decided 2026-11-04)

**Owner question:** Q-01 approved Admin tokens memory-only. P-01 left open whether the desktop app keeps its keyring-backed session or changes it. Decided here by the coordinator under the owner's standing authority to decide and document. The owner may override by a new entry.

## Decision

1. **Desktop keeps the OS keyring, but stores the refresh token only.** The access token lives in memory and is never written to disk.
2. **Desktop may restore a session at startup** from the keyring by exchanging the refresh token for a new access token. If the exchange fails, the app shows the login screen. No failure is fatal.
3. **Admin stays memory-only** (Q-01). Admin tokens are never written to disk, the keyring, or browser storage. Closing the app ends the session.
4. **Logout clears both** the in-memory access token and the keyring entry, and revokes the refresh token on the server where the Worker supports it.
5. **Keyring failure is non-fatal.** If the keyring is unavailable, the desktop app runs signed-in for the current launch only and shows a notice. It never falls back to plain files.
6. **Nothing else is stored:** no passwords, no access tokens, no tokens in logs, crash reports, diagnostics, Slint properties or event payloads (P-01 contract, C-08).

## Why

- **Desktop is an operations app used across restarts.** Forcing a login on every launch is a real usability cost for staff. Storing only the refresh token in the OS keyring keeps that cost low.
- **The access token is short-lived** (one hour in the Worker, per the current-state doc), so keeping it in memory limits exposure.
- **The OS keyring is protected by the user's OS account.** It is safer than files or browser storage, and it is the existing desktop mechanism (`KeyringSecureStorage`), so no new storage path is added.
- **Admin is different.** Admin manages organization-wide data and is the most sensitive role. Memory-only means a stolen disk or a lost laptop does not leave a usable session behind. The owner already approved this in Q-01.
- **Refresh-token-only storage** means a leaked keyring entry is still bounded by the refresh token's server-side lifetime and revocation, not a long-lived access credential.

## What must change to implement this

- `StoredSession` currently stores `access_token` and `refresh_token`. The stored form must drop `access_token`. Restore must call the refresh endpoint first.
- The refresh endpoint's exact contract (route, request, response, expiry, revocation) must be verified from the Worker before implementation. P-01 Track A says not to invent it.
- Tests: restore success, restore with refresh failure (login screen, no crash), keyring unavailable (notice, signed in for the launch), logout clears keyring and memory, no access token ever written.

## Open

- Whether the Worker revokes the refresh token on logout. To be verified in Track A.
- Relay-only fallback when LAN discovery fails: still open, owner decision (see P-01 evidence summary, section 5).
