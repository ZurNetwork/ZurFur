-- Commission positioning: two account-facing rails, kept in separate tables,
-- that neither confer any in-commission authority. Both are commission-owned
-- bookkeeping, not facts, so they cascade away with the commission or the
-- account rather than blocking deletion. See NODE.md
-- ("20260705200000_create_commission_placement_and_view_grant.sql") for the
-- full rationale. (These two tables were later dropped and replaced by
-- workflow/column placement — see `20260910193912_drop_superseded_placement_tables.sql`.)

-- The append-only placement log: one row per (re)placement. The
-- log is NEVER rewritten. seq is the monotonic ordering key (a
-- surrogate bigserial, matching the changelog/plc_operations Postgres-as-log
-- precedent): the CURRENT placement is the greatest seq, the ORIGIN the least.
--
-- placed_by     The User who placed it (the commission owner in v1). Deliberately
--               NO foreign key onto users(id) — matching the changelog's actor_id:
--               positioning history must not be blocked by, nor cascade into, a
--               future user-row removal.
CREATE TABLE commission_placement (
    seq           bigserial   PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    account_id    uuid        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    placed_by     uuid        NOT NULL,
    placed_at     timestamptz NOT NULL
);

-- The one read: a commission's placement log in append order.
CREATE INDEX commission_placement_commission_seq ON commission_placement (commission_id, seq);

-- The denormalized CURRENT-placement pointer: exactly one row per
-- placed commission, upserted in the SAME unit of work as each log append, so it
-- always equals the latest log row (never a second transaction). Kept apart from
-- the log so "current" is an O(1) read and the invariant (pointer == latest row)
-- is directly testable.
CREATE TABLE commission_current_placement (
    commission_id uuid        PRIMARY KEY REFERENCES commission (id) ON DELETE CASCADE,
    account_id    uuid        NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    seq           bigint      NOT NULL,
    placed_by     uuid        NOT NULL,
    placed_at     timestamptz NOT NULL
);

-- The commission-side view grant: a pure KEY to see, at an explicitly chosen
-- level. At most one key per (commission, account) — the composite
-- primary key — so re-granting REPLACES the level (upsert, "issuing anew"). A key
-- HARD-DELETES on revoke: no soft-deleted rows. Deliberately just the
-- level: the grant is a PURE KEY, and its history — who issued it, when, and who
-- revoked it — lives ONLY in the changelog, so a revoked key stops
-- lifting on the next server-side serialization by construction.
--
-- level         The GrantLevel token, validated by the domain enum (the closed
--               vocabulary presentation/description/total; text, not a pg enum, so
--               adding a mode is not a migration — though the domain rule fixes it at three).
CREATE TABLE commission_view_grant (
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    account_id    uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    level         text NOT NULL,
    PRIMARY KEY (commission_id, account_id)
);
