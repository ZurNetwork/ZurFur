UPDATE accounts
SET deleted_at = $1, updated_at = $1
WHERE id = $2 AND deleted_at IS NULL
