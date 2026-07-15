-- params: id
-- fetch: optional
-- row: ActorIdentityRow
SELECT id, kind
FROM actor_identity
WHERE id = $1
