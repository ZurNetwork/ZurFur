UPDATE accounts
SET handle = $1, updated_at = $2
WHERE id = $3 AND deleted_at IS NULL AND handle = $4
