-- params: id
-- fetch: optional
-- row: ActorIdentityRow
SELECT id, kind, did, state, handle
FROM actor_identity
WHERE id = $1
