-- Rename one column. The (workflow_id, name) unique constraint is the store-level
-- backstop for the board-level check `Workflow::rename_column` already made.
UPDATE workflow_column SET name = $2 WHERE id = $1
