-- params: id, kind, did, state
-- fetch: one
-- row: ActorIdentityRow
INSERT INTO actor_identity (id, kind, did, state)
VALUES ($1, $2, $3, $4)
ON CONFLICT (did) DO UPDATE SET did = EXCLUDED.did
RETURNING id, kind, did, state, handle
