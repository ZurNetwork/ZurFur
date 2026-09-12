-- One board's own row. Its columns are a separate read (`columns.sql`), because
-- `Workflow::loaded` wants them already ordered and card-filled.
SELECT account_id, name, visibility
FROM workflow
WHERE id = $1
