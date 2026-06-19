-- Read-through cache of public profiles (see ZMVP-10, DESIGN/User). Handle,
-- display name, and avatar are user-owned data on the public boundary (the PDS);
-- we cache them privately so repeat views don't need the PDS awake. We read and
-- cache, we never own — this row is a copy, freely discardable and rebuildable.
--
-- did          The visitor's did:plc — the natural key (one cached profile per
--              DID). Stored as the AT identity, not an app-minted UUID, because
--              the cache is keyed by the public identity it mirrors.
-- handle       Resolved from the DID document; always present.
-- display_name Optional: a PDS may carry none.
-- avatar_url   Optional: absence is not an error (graceful degradation).
-- fetched_at   When we last read this from the PDS; drives the freshness/TTL
--              policy. A read past the TTL is treated as a miss and refetched.
CREATE TABLE profile_cache (
    did          text        PRIMARY KEY,
    handle       text        NOT NULL,
    display_name text,
    avatar_url   text,
    fetched_at   timestamptz NOT NULL
);
