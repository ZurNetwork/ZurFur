-- params: did
-- fetch: optional
-- row: ActorIdentityRow
SELECT id, kind, did, state
FROM actor_identity
WHERE did = $1
