-- Resolve a DID to its User via the actor super-table (ZMVP-123: the DID lives there
-- now, not on `users`). The caller already holds the DID it looked up, so the row need
-- only carry the projection's own columns — the adapter pairs them with that DID.
-- LIMIT 1 is the codegen at-most-one signal: the unique cover (PK/UNIQUE)
-- sits behind the actor_identity join, which the pg_index cardinality proof
-- cannot cross — without it the generated contract degrades to Vec<T>.
SELECT u.id, u.created_at
FROM users u
JOIN actor_identity ai ON ai.id = u.id
WHERE ai.did = $1
LIMIT 1
