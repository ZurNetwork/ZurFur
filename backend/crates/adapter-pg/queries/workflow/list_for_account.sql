-- The Workflows one Account owns: id and name only, ordered by id (UUIDv7, so
-- creation order). Seeks `workflow_account`.
SELECT id, name
FROM workflow
WHERE account_id = $1
ORDER BY id
