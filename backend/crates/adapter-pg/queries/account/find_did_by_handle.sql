-- Resolve a live account's handle to its DID (ZMVP-123: the DID lives in the actor
-- super-table now, joined on the shared id). `handle` stays the authoritative,
-- globally unique claim on `accounts`; this backs the `/.well-known/atproto-did`
-- resolver and the founding duplicate-handle pre-check. An account always has a DID,
-- so `ai.did IS NOT NULL` is a true no-op that keeps the scalar result a plain DID.
-- LIMIT 1 is the codegen at-most-one signal: the unique cover (PK/UNIQUE)
-- sits behind the actor_identity join, which the pg_index cardinality proof
-- cannot cross — without it the generated contract degrades to Vec<T>.
SELECT ai.did
FROM accounts a
JOIN actor_identity ai ON ai.id = a.id
WHERE a.handle = $1 AND a.deleted_at IS NULL AND ai.did IS NOT NULL
LIMIT 1
