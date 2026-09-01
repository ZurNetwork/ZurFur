-- The account that owns the board this column sits on — the authorization
-- lookup a column mutation makes when it holds only the column's id.
SELECT w.account_id
FROM workflow_column c
JOIN workflow w ON w.id = c.workflow_id
WHERE c.id = $1
