INSERT INTO users (id, did, created_at)
VALUES ($1, $2, $3)
ON CONFLICT (did) DO UPDATE SET did = EXCLUDED.did
RETURNING id, did, created_at
