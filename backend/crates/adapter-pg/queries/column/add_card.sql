-- Place one card in one column at one index — the second half of the wholesale
-- rewrite. The (column_id, position) unique constraint is DEFERRABLE, so the
-- rewrite may pass through duplicate indexes and is checked once at COMMIT.
INSERT INTO workflow_column_commission (column_id, commission_id, position)
VALUES ($1, $2, $3)
