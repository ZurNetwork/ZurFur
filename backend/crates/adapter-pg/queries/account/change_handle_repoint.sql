-- handle receives the new handle; acc_handle guards the expected current
-- handle, so a concurrent rename loses.
UPDATE accounts AS acc
SET handle = $1, updated_at = $2
WHERE acc.id = $3 AND acc.deleted_at IS NULL AND acc.handle = $4
