-- params: id
-- fetch: optional
SELECT id
FROM actor_identity
WHERE id = $1
