-- Every LIVE account `$1` holds a role in, together with that role (ZMVP-157) —
-- not owned-only: gaining a role is how a user joins an account on this
-- platform, so an accepted invitation belongs here exactly as a founded
-- account does. Keeps `find`'s `deleted_at IS NULL` liveness filter; since the
-- actor re-key (DD 57081857) `accounts.id` IS the DID, so the actor_identity
-- join that used to recover one is gone.
--
-- ORDER BY … COLLATE "C" sorts the DID by byte value, which is what Rust's
-- `str` ordering does — the adapter-mem twin sorts the same list in process, and
-- the two must not disagree because the server was initdb'd under a different
-- locale. (Creation order is no longer available from the key: a DID carries no
-- timestamp, where the retired UUIDv7 did.)
--
-- `$2` gates the `listed_on_profile` privacy valve (DD 21594113 decision 4).
-- TRUE for a PUBLIC projection of this user's memberships, which shows only
-- the ones they chose to publish; FALSE for the member's OWN view, which shows
-- every live membership — a member's own records are not hidden from them by
-- their own publication choice. The domain side takes this as a required
-- `ListingScope`, so the valve cannot be bypassed by omission.
--
-- $2: honor_privacy
SELECT a.id, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at, am.role, am.alias
FROM account_members am
JOIN accounts a ON a.id = am.account_id
WHERE am.user_id = $1
  AND a.deleted_at IS NULL
  AND (am.listed_on_profile OR NOT $2::boolean)
ORDER BY a.id COLLATE "C"
