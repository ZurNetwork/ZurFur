-- params: did
-- fetch: optional
-- row: UserRow
SELECT id, did, created_at
FROM users
WHERE did = $1
