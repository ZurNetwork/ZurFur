-- Every LIVE account `$1` holds a role in, together with that role (ZMVP-157) —
-- not owned-only: gaining a role is how a user joins an account on this
-- platform, so an accepted invitation belongs here exactly as a founded
-- account does. Mirrors `find`'s DID join (ZMVP-123) and its
-- `deleted_at IS NULL` liveness filter. Ordered by account id (UUIDv7 sorts
-- as creation order).
--
-- `$2` gates the `listed_on_profile` privacy valve (DD 21594113 decision 4).
-- TRUE for a PUBLIC projection of this user's memberships, which shows only
-- the ones they chose to publish; FALSE for the member's OWN view, which shows
-- every live membership — a member's own records are not hidden from them by
-- their own publication choice. The domain side takes this as a required
-- `ListingScope`, so the valve cannot be bypassed by omission.
--
-- $2: honor_privacy
SELECT a.id, ai.did, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at, am.role
FROM account_members am
JOIN accounts a ON a.id = am.account_id
JOIN actor_identity ai ON ai.id = a.id
WHERE am.user_id = $1
  AND a.deleted_at IS NULL
  AND (am.listed_on_profile OR NOT $2::boolean)
ORDER BY a.id
