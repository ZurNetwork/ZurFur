-- The account's DID now lives in the actor super-table (ZMVP-123): join it back on the
-- shared id. An account always has a DID (the per-kind CHECK on actor_identity), so a
-- NULL here would be a corrupted projection — the adapter surfaces it as an error.
-- LIMIT 1 is the codegen at-most-one signal: the unique cover (PK/UNIQUE)
-- sits behind the actor_identity join, which the pg_index cardinality proof
-- cannot cross — without it the generated contract degrades to Vec<T>.
SELECT a.id, ai.did, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at
FROM accounts a
JOIN actor_identity ai ON ai.id = a.id
WHERE a.id = $1
  AND a.deleted_at IS NULL
LIMIT 1
