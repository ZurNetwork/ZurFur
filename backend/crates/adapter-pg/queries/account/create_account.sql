-- The account's DID now lives in the actor super-table (ZMVP-123): the caller interns
-- it (keyed by this same id) as the first step of the create unit, so the projection
-- row carries no `did`. `kind` is filled by its constant column DEFAULT ('account')
-- and never named here. `handle` STAYS on accounts — it is the authoritative, globally
-- unique resolution claim, not the actor_identity display cache.
INSERT INTO accounts (id, handle, name, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5)
