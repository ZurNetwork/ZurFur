-- deleted_at and updated_at both receive the deletion instant ("now").
UPDATE accounts
SET deleted_at = $1, updated_at = $2
WHERE id = $3 AND deleted_at IS NULL
