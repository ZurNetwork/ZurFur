-- params: id
-- fetch: optional
-- row: ActorIdentityRow
SELECT id, kind, did, state, handle, first_seen
FROM actor_identity
WHERE id = $1
