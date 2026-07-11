SELECT id, did, created_at
FROM users
WHERE did = $1
