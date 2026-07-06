-- The commission content tree (ZMVP-71; Surfaces DD DESIGN/28246028, Tree
-- Storage DD DESIGN/28409880): one adjacency row per node — envelope as real
-- columns Postgres can constrain and audit, payload as opaque jsonb the core
-- never interprets. Whole-tree read model: one indexed query per commission,
-- assembly and projection in Rust (no ltree, no closure table, no recursive
-- CTEs).
--
-- id             The node key. App-minted UUIDv7 for grown nodes; the backfill
--                below mints v4 (PG16 has no uuidv7(); a singleton root doesn't
--                need time-sortability — created_at carries the real time).
-- commission_id  The tree this node belongs to. ON DELETE CASCADE: nodes are
--                commission-owned bookkeeping, NOT facts (Deletion DD 3014657)
--                — the whole tree goes when the commission hard-deletes
--                (ZMVP-66 relies on this cascade).
-- parent         Adjacency: the parent node, NULL = the root surface. The
--                self-referential ON DELETE CASCADE is what makes subtree
--                removal (ZMVP-73) one row delete.
-- type           The envelope's type tag: 'surface' now; component type tags
--                arrive with ZMVP-72+ (text, not a pg enum — the catalog DD
--                types it later without migrations).
-- mode           A surface's visibility mode ('presentation' | 'description' |
--                'total'); NULL on components, which inherit their parent's
--                (Surfaces DD amendment). The CHECK gives that rule teeth: a
--                surface ALWAYS carries a mode, anything else NEVER does. The
--                root's mode is the commission-level visibility itself.
-- position       Sibling order within the parent, renumbered in-transaction on
--                insert-between (append = max + 1). DEFERRABLE so a renumbering
--                UPDATE may pass through intermediate collisions inside one
--                transaction.
-- created_by     The acting User — per-node FK teeth for authorization/audit
--                (Tree Storage DD: plugin subtree isolation reads this).
-- created_at     When the node was created. Application-supplied (no DEFAULT
--                now()), matching the codebase convention.
-- payload        The type-owned half of the node, schemaless at the DB layer by
--                design; validation lives with the future type catalog.
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

-- Root backfill (ZMVP-71 AC1, retroactive half): every commission created
-- before the tree existed gets its root surface here, mode mapped from the flat
-- visibility column exactly as the Surfaces DD amendment aliases it —
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
