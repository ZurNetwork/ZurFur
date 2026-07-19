-- The visitor's DID now lives in the actor super-table (ZMVP-123): join it back on
-- the shared id. A User always has a DID (the per-kind CHECK on actor_identity), so a
-- NULL here would be a corrupted projection — the adapter surfaces it as an error.
-- LIMIT 1 is the codegen at-most-one signal: the unique cover (PK/UNIQUE)
-- sits behind the actor_identity join, which the pg_index cardinality proof
-- cannot cross — without it the generated contract degrades to Vec<T>.
SELECT u.id, ai.did, u.created_at
FROM users u
JOIN actor_identity ai ON ai.id = u.id
WHERE u.id = $1
LIMIT 1
