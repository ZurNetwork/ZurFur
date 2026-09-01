-- Which column of a given board holds a card, if any. A commission sits in at
-- most one column per board, but on as many boards as care to position it — the
-- NxM the Ownership Separation DD makes native (D1/D6), so this is deliberately
-- scoped by workflow and never answers "the" column of a commission.
SELECT c.id, c.workflow_id, c.name, c.visibility, c.position
FROM workflow_column c
JOIN workflow_column_commission cc ON cc.column_id = c.id
WHERE c.workflow_id = $1 AND cc.commission_id = $2
