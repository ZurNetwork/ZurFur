-- params: id
-- fetch: optional
-- row: UserRow
SELECT id, did, created_at
FROM users
WHERE id = $1
