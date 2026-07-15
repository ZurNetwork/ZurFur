-- actor_identity slice 4 (ZMVP-122, DD 34013187 decisions 3/5): the liveness
-- state column. Every row is born active; pulled/tombstoned are the designed
-- non-deletion endings — the state machine and read-path predicate are
-- ZMVP-125's ticket, this slice only gives the state a column to live in.
-- DEFAULT backfills any pre-existing rows, then drops: the value is
-- application-supplied thereafter, matching the codebase convention.
ALTER TABLE actor_identity
    ADD COLUMN state text NOT NULL DEFAULT 'active'
        CHECK (state IN ('active', 'pulled', 'tombstoned'));

ALTER TABLE actor_identity
    ALTER COLUMN state DROP DEFAULT;
