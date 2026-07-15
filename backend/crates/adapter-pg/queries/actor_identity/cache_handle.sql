-- params: id, handle?
-- fetch: execute
UPDATE actor_identity
SET handle = $2
WHERE id = $1
