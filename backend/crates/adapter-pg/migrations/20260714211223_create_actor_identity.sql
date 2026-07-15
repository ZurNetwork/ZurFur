-- actor_identity: the actor super-table (Party pattern) — DD 34013187 / ZMVP-122.
-- Built incrementally: this slice is existence only. kind / did / handle / state /
-- first_seen arrive in later slices, each with its own tests.
-- Rows are immortal — no DELETE path, ever (liveness will be a state, not a removal).
CREATE TABLE actor_identity (
    id uuid PRIMARY KEY  -- App-minted UUIDv7 (PG16 has no native uuidv7())
);
