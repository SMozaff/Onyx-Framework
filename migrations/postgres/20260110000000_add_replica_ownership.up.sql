-- Audit finding H7: binds a Cloud Relay replica identity (self_replica) to
-- the first authenticated user who legitimately claims it, so a relay
-- ticket can never be minted letting a different user register as a
-- replica someone else already owns. First-claim-wins, enforced by the
-- primary key: two concurrent first claims for the same replica_id can
-- only ever let one INSERT actually land.
CREATE TABLE replica_ownership (
    replica_id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    organization_id UUID NOT NULL,
    claimed_at BIGINT NOT NULL
);
CREATE INDEX idx_replica_ownership_user ON replica_ownership (user_id);
