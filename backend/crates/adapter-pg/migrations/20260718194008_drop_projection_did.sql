-- Drop the DID duplicates from the
-- projection tables. `actor_identity.did` is now the single home of the actor's DID
-- (backfilled earlier, guaranteed present for user/account by an existing CHECK
-- and UNIQUE across all kinds by the super-table index). Every read that needs a
-- user's/account's DID now joins the super-table on the shared id; nothing FKs onto
-- `users.did`/`accounts.did` (all references target `(id)`), so the columns — and
-- their now-redundant UNIQUE indexes — drop cleanly.
--
-- HANDLE STAYS. `accounts.handle` is the AUTHORITATIVE, globally-UNIQUE, claim-
-- validated resolution key (`accounts_handle_key`; the `/.well-known/atproto-did`
-- resolver, the founding 409, and the change-flow quarantine all rely on it).
-- It is NOT the same fact as `actor_identity.handle`, which is
-- a NON-unique, never-claim-validated display cache. Collapsing the two would
-- forfeit handle uniqueness and resolution integrity, so handle is deliberately left
-- in place. (`users` never had a handle column.)

ALTER TABLE users DROP COLUMN did;
ALTER TABLE accounts DROP COLUMN did;
