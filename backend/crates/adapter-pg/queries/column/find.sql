-- One column's own row. Its cards are a separate read (`cards.sql`), because
-- `Column::loaded` takes them already ordered.
SELECT workflow_id, name, visibility, position
FROM workflow_column
WHERE id = $1
