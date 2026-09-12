-- A board's columns in board order. Ordered by the base-62 fractional key,
-- compared BYTEWISE — the column is COLLATE "C", so this ordering is the same
-- one `Position` mints against (`Workflow::loaded` refuses keys that do not
-- strictly ascend, so a collation mismatch surfaces as an error, not a scramble).
SELECT id, name, visibility, position
FROM workflow_column
WHERE workflow_id = $1
ORDER BY position
