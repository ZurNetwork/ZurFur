-- The commission content tree (later retired — see
-- `20260805234354_flat_commission_composition.sql`): one adjacency row per
-- node — envelope as real columns Postgres can constrain and audit, payload
-- as opaque jsonb the core never interprets. Whole-tree read model: one
-- indexed query per commission, assembly and projection in Rust. See NODE.md
-- ("20260705150000_create_commission_node.sql") for the column-by-column
-- rationale.
CREATE TABLE commission_node (
    id            uuid        PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    parent        uuid        REFERENCES commission_node (id) ON DELETE CASCADE,
    type          text        NOT NULL,
    mode          text,
    position      integer     NOT NULL,
    created_by    uuid        NOT NULL REFERENCES users (id),
    created_at    timestamptz NOT NULL,
    payload       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    UNIQUE (parent, position) DEFERRABLE INITIALLY DEFERRED,
    CHECK ((type = 'surface') = (mode IS NOT NULL))
);

-- Exactly one root per commission: a second parentless row is unrepresentable.
CREATE UNIQUE INDEX one_root_per_commission
    ON commission_node (commission_id) WHERE parent IS NULL;

-- The whole-tree read: SELECT … WHERE commission_id = $1.
CREATE INDEX commission_node_by_commission ON commission_node (commission_id);

-- gen_random_uuid() is core from PG13 but lives in pgcrypto on older servers
-- (the test containers run one); harmless where it's already core.
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Root backfill: every commission created
-- before the tree existed gets its root surface here, mode mapped from the flat
-- visibility column —
-- private -> total, listed -> presentation, public -> description. The CASE is
-- deliberately ELSE-less: a visibility token outside the vocabulary would map
-- to NULL, violate the surface-has-a-mode CHECK, and abort the migration loudly
-- — tampering never becomes a silently-widened (or -narrowed) root. New
-- commissions never reach this path: their root is minted in the same unit of
-- work as the commission row (CommissionWrites::create).
INSERT INTO commission_node
    (id, commission_id, parent, type, mode, position, created_by, created_at, payload)
SELECT
    gen_random_uuid(),
    c.id,
    NULL,
    'surface',
    CASE c.visibility
        WHEN 'private' THEN 'total'
        WHEN 'listed'  THEN 'presentation'
        WHEN 'public'  THEN 'description'
    END,
    0,
    c.owner_id,
    c.created_at,
    '{}'::jsonb
FROM commission c;
