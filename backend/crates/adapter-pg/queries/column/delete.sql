-- Delete one column. Its card edges go with it (ON DELETE CASCADE); the
-- commissions they pointed at are untouched. The caller refuses a column that
-- still holds cards, so the cascade is a backstop, not the path.
DELETE FROM workflow_column WHERE id = $1
