-- The write-side surface-tree depth cap (ZMVP-164; Surface Tree on the Wire
-- DD DESIGN/42762241's companion ruling): a stored tree must never grow
-- deeper than the v1 wire transport can decode back. DD 40992770 makes
-- JSON/HTTP the v1 transport (protobuf is the IDL, not what ships in v1), so
-- the binding ceiling is `serde_json`'s recursion guard, not `prost`'s — the
-- pinned `prost` build's `RECURSION_LIMIT` is 100, comfortably clear of
-- anything this cap needs. `serde_json` defaults `remaining_depth` to 128,
-- and each tree level costs TWO brackets on the wire (the children ARRAY
-- plus the child OBJECT: `"children":[{...}]`), so 128 / 2 = 64 levels is
-- the real ceiling. A 63-deep tree spends ~127 of those 128 brackets —
-- exactly ONE BRACKET of headroom, not one whole level. (Corrected
-- 2026-07-30: an earlier draft of this migration cited a flat "prost dies at
-- 64 levels", which was wrong on both which library's limit matters and
-- what "64" actually measures.)
--
-- depth        The node's own nesting level, counting the root surface as
--              level 1 (so it lines up 1:1 with the wire
--              `SurfaceTree`/`SurfaceNode` nesting the DD mints — one
--              `SurfaceNode` message per level): the root is `1`, its
--              children `2`, and so on, capped at `63` by the bounds CHECK
--              below.
-- child_depth  A GENERATED column carrying `depth + 1` — the depth ANY
--              child of this row must hold. It exists solely to be the
--              target half of a composite FK (DD 34013187's
--              "unrepresentable, not merely checked" idiom; model
--              `20260718194001_actor_projection_composite_fk.sql`): paired
--              with `UNIQUE (id, child_depth)` below, a child row's own
--              `FOREIGN KEY (parent, depth) REFERENCES commission_node (id,
--              child_depth)` can be satisfied ONLY when its `depth` equals
--              its parent's `depth + 1`. A node whose stored `depth`
--              disagrees with its parent's isn't merely rejected by a
--              CHECK that inspects it after the fact — there is no row in
--              the universe of valid foreign keys for it to reference, so
--              it is unrepresentable. This composite FK REPLACES the plain
--              self-referential FK the original migration put on `parent`
--              (`commission_node_parent_fkey`, dropped below) — `ON DELETE
--              CASCADE` still cascades on `id`, so subtree removal
--              (ZMVP-73) is unaffected.
--
-- Together with the bounds CHECK, the composite FK IS the real backstop the
-- application guard (`PgCommissionWrites::require_surface_parent`) only
-- echoes at the domain layer: even a raw write that bypasses the domain
-- entirely cannot construct a node whose `depth` disagrees with its
-- parent's, nor one past level 63 — both are now unforgeable, not merely
-- checked.
--
-- The backfill below is a ONE-TIME recursive walk of whatever tree shape
-- already exists (this table has taken writes since ZMVP-71/72/76/77
-- shipped) — unrelated to the Tree Storage DD's "no recursive CTEs" stance,
-- which is about the RUNTIME whole-tree read model (one indexed query,
-- assembly in Rust); a migration-time, run-once backfill is ordinary
-- practice and never runs again after this file lands.
--
-- Pre-alpha, so there is no remediation path for legacy rows that would
-- violate the constraints below — but a bare constraint-name error on boot
-- is undiagnosable, so this preflight names the offending commissions in the
-- server log (one cheap recursive scan, read-only) before the ALTERs below
-- get a chance to abort opaquely.
DO $$
DECLARE
    orphaned uuid[];
    overdeep uuid[];
BEGIN
    WITH RECURSIVE node_depth AS (
        SELECT id, 1 AS depth FROM commission_node WHERE parent IS NULL
        UNION ALL
        SELECT n.id, node_depth.depth + 1
        FROM commission_node n
        JOIN node_depth ON n.parent = node_depth.id
    )
    SELECT
        array_agg(DISTINCT c.commission_id) FILTER (WHERE d.id IS NULL),
        array_agg(DISTINCT c.commission_id) FILTER (WHERE d.depth > 63)
    INTO orphaned, overdeep
    FROM commission_node c
    LEFT JOIN node_depth d ON d.id = c.id;

    IF orphaned IS NOT NULL THEN
        RAISE NOTICE 'commission_node_depth_cap preflight: detached or cyclic tree rows in commission(s) % — will fail the coming depth NOT NULL backfill', orphaned;
    END IF;
    IF overdeep IS NOT NULL THEN
        RAISE NOTICE 'commission_node_depth_cap preflight: tree already exceeds level 63 in commission(s) % — will fail the coming depth bounds CHECK', overdeep;
    END IF;
END $$;

ALTER TABLE commission_node ADD COLUMN depth integer;

WITH RECURSIVE node_depth AS (
    SELECT id, 1 AS depth FROM commission_node WHERE parent IS NULL
    UNION ALL
    SELECT n.id, node_depth.depth + 1
    FROM commission_node n
    JOIN node_depth ON n.parent = node_depth.id
)
UPDATE commission_node
SET depth = node_depth.depth
FROM node_depth
WHERE commission_node.id = node_depth.id;

ALTER TABLE commission_node ALTER COLUMN depth SET NOT NULL;
ALTER TABLE commission_node ADD CONSTRAINT commission_node_depth_bounds
    CHECK (depth >= 1 AND depth <= 63);
ALTER TABLE commission_node ADD CONSTRAINT commission_node_root_depth_is_one
    CHECK (parent IS NOT NULL OR depth = 1);

ALTER TABLE commission_node
    ADD COLUMN child_depth integer GENERATED ALWAYS AS (depth + 1) STORED;
ALTER TABLE commission_node ADD CONSTRAINT commission_node_id_child_depth_key
    UNIQUE (id, child_depth);

ALTER TABLE commission_node DROP CONSTRAINT commission_node_parent_fkey;
ALTER TABLE commission_node ADD CONSTRAINT commission_node_parent_depth_fk
    FOREIGN KEY (parent, depth) REFERENCES commission_node (id, child_depth)
    ON DELETE CASCADE;
