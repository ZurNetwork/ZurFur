-- Commission positioning (ZMVP-70; Ownership Separation DD DESIGN/29130754): the
-- two account-facing rails that replaced the deleted managing-account concept.
-- Users own commissions; accounts own positioning, and NEITHER rail confers any
-- in-commission authority (DD Decision 8, the environmental rule). Placement is
-- account-side, the view grant is a commission-side key — they NEVER share a
-- table (Decision 6). Both are commission-owned bookkeeping, NOT facts (Deletion
-- DD 3014657): they cascade away with the commission (registered in
-- COMMISSION_NON_FACT_TABLES), so a hard-delete (ZMVP-66) reaps them, never blocks
-- on them.
--
-- Every account FK is ON DELETE CASCADE as well: a placement/grant references a
-- LIVE account, so account hard-delete (ZMVP-34's explicit ordered child deletes)
-- would FK-violate on these new tables without it. Cascading keeps that path
-- sound AND pre-implements ZMVP-57's hard-delete severance (placements + grants
-- gone, commission untouched); ZMVP-57 still owns the SOFT-delete "a deactivated
-- account's key stops conferring" read-side behavior and the account_has_facts
-- tripwire.

-- The append-only placement log: one row per (re)placement (Decision 1/6). The
-- log is NEVER rewritten (ZMVP-70 AC2). seq is the monotonic ordering key (a
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

-- The denormalized CURRENT-placement pointer (ZMVP-70 AC3): exactly one row per
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
-- level (Decision 3). At most one key per (commission, account) — the composite
-- primary key — so re-granting REPLACES the level (upsert, "issuing anew"). A key
-- HARD-DELETES on revoke (Decision 5): no soft-deleted rows. Deliberately just the
-- level: the grant is a PURE KEY, and its history — who issued it, when, and who
-- revoked it — lives ONLY in the changelog (Decision 5), so a revoked key stops
-- lifting on the next server-side serialization by construction.
--
-- level         The GrantLevel token, validated by the domain enum (the closed
--               vocabulary presentation/description/total; text, not a pg enum, so
--               adding a mode is not a migration — though the DD fixes it at three).
CREATE TABLE commission_view_grant (
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    account_id    uuid NOT NULL REFERENCES accounts (id) ON DELETE CASCADE,
    level         text NOT NULL,
    PRIMARY KEY (commission_id, account_id)
);
