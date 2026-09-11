-- Resolve a live account's handle to its DID. Since the actor re-key (DD 57081857)
-- the account's id IS its DID, so this is a plain lookup on `accounts` — the
-- actor_identity join it used to need is gone with the surrogate key. `handle` stays
-- the authoritative, globally unique claim; this backs the `/.well-known/atproto-did`
-- resolver and the founding duplicate-handle pre-check.
SELECT a.id
FROM accounts a
WHERE a.handle = $1 AND a.deleted_at IS NULL
