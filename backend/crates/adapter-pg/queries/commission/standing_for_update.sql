-- `standing`'s commission facets, with the commission row locked until the unit
-- of work ends: the same probe inner-joined, so an absent id answers no row.
-- FOR NO KEY UPDATE OF commission locks the commission row alone. Every change to
-- someone's standing locks that row first, so it waits for this unit. Holds no
-- participant probe: a probe in this statement reads the snapshot taken before
-- the lock was granted, so the caller runs `is_participant` after it instead.
-- $1: commission_id
-- $2: viewer
SELECT commission.visibility,
       commission.owner_id = probe.viewer AS is_owner,
       commission.archived_at IS NOT NULL AS archived
FROM (VALUES ($1::uuid, $2::text)) AS probe (commission_id, viewer)
JOIN commission ON commission.id = probe.commission_id
FOR NO KEY UPDATE OF commission
