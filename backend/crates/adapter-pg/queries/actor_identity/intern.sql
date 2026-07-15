-- params: id, kind, did, state, first_seen
-- fetch: one
-- row: ActorIdentityRow
INSERT INTO actor_identity (id, kind, did, state, first_seen)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (did) DO UPDATE SET did = EXCLUDED.did
RETURNING id, kind, did, state, handle, first_seen
