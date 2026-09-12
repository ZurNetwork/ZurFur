-- Persist one column of a board as the domain holds it — the per-column half of
-- `set_indexes`. An UPSERT because a board write is never "insert" or "update"
-- from the caller's side: `Columns::add` mints a column into the in-memory board
-- and hands the WHOLE board over, so the new column and its displaced
-- neighbours' keys arrive through one path.
INSERT INTO workflow_column (id, workflow_id, name, visibility, position)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (id) DO UPDATE
SET name = EXCLUDED.name,
    visibility = EXCLUDED.visibility,
    position = EXCLUDED.position
