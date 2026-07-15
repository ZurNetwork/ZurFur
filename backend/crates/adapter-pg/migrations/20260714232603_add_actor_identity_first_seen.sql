-- actor_identity slice 7 (ZMVP-122, DD 34013187 decision 2): first_seen — when
-- the Index first saw this actor. Application-supplied (no DEFAULT now()),
-- matching the codebase convention; the ADD-time DEFAULT only backfills any
-- pre-existing rows and is dropped immediately. Immutable by contract: set at
-- create/intern, never updated (re-interning a DID keeps the original stamp).
ALTER TABLE actor_identity
    ADD COLUMN first_seen timestamptz NOT NULL DEFAULT now();

ALTER TABLE actor_identity
    ALTER COLUMN first_seen DROP DEFAULT;
