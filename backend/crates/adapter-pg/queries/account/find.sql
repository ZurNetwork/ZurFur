-- params: id
-- fetch: optional
-- row: AccountRow
SELECT id, did, handle, name, created_at, updated_at, deleted_at
FROM accounts
WHERE id = $1
  AND deleted_at IS NULL
