SELECT id, kind, did, state, handle, first_seen
FROM actor_identity
WHERE did = $1
