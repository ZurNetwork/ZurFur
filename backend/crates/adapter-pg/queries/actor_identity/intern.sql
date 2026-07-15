-- params: id, kind, did
-- fetch: one
-- row: ActorIdentityRow
INSERT INTO actor_identity (id, kind, did)
VALUES ($1, $2, $3)
ON CONFLICT (did) DO UPDATE SET did = EXCLUDED.did
RETURNING id, kind, did
