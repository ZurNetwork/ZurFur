-- The write-side surface-tree depth cap (ZMVP-164; Surface Tree on the Wire
-- DD DESIGN/42762241's companion ruling): a stored tree must never grow
-- deeper than either wire tier can decode back — Rust's `prost` dies at 64
-- nested `SurfaceNode` levels, TypeScript's protobuf-es at ~100. The binding
-- decode ceiling is Rust's, so the write path caps at 63.
--
-- depth  The node's own nesting level, counting the root surface as level 1
--        (so it lines up 1:1 with the wire `SurfaceTree`/`SurfaceNode`
--        nesting the DD mints — one `SurfaceNode` message per level): the
--        root is `1`, its children `2`, and so on. A node may only be
--        inserted when its parent's depth is `< 63`, so no write can ever
--        produce a level-64 node — the level Rust's decoder cannot read
--        back. The CHECK below is the database-level backstop for the same
--        rule the application guard (`PgCommissionWrites::require_surface_parent`)
--        enforces before every insert — belt and suspenders, not the only gate.
--
-- The backfill below is a ONE-TIME recursive walk of whatever tree shape
-- already exists (this table has taken writes since ZMVP-71/72/76/77
-- shipped) — unrelated to the Tree Storage DD's "no recursive CTEs" stance,
-- which is about the RUNTIME whole-tree read model (one indexed query,
-- assembly in Rust); a migration-time, run-once backfill is ordinary
-- practice and never runs again after this file lands.
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
