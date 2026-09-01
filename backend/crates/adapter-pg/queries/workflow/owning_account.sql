-- The account a board belongs to — the authorization lookup every board
-- mutation makes before touching anything (a workflow belongs to exactly one
-- account, DESIGN/Workflow).
SELECT account_id
FROM workflow
WHERE id = $1
