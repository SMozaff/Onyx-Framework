-- Audit finding H-02: durable, shared session/token revocation, replacing
-- the former in-process-only ApiState::revoked_tokens. Two tables for two
-- distinct operations -- see security_application::ports::token_revocation
-- for why both are needed.

-- Individual-token revocation (logout, refresh-token rotation). Keyed by a
-- hash of the token, never the raw token value.
CREATE TABLE revoked_tokens (
    token_hash TEXT PRIMARY KEY,
    revoked_at BIGINT NOT NULL
);

-- Per-user watermark: every token issued (iat) before revoked_before is
-- invalid for this user. Used where the server never tracked which
-- individual tokens are outstanding (user deactivation, admin-driven
-- password reset) and must invalidate every session at once.
CREATE TABLE user_token_revocations (
    user_id UUID PRIMARY KEY,
    revoked_before BIGINT NOT NULL
);
