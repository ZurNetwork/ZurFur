-- The commissions a User is a Participant of, read from the same
-- `commission_participant` record `is_participant` reads, each with its node's
-- name and level; a commission with no node is left out. Archived commissions
-- are included; hiding them is the caller's. Seeks the by-user index; ordered
-- by commission id (UUIDv7 sorts as creation order). The archive time itself
-- is never decoded, only whether there is one.
SELECT c.id, n.name, n.level, c.archived_at IS NOT NULL AS archived
FROM commission_participant p
JOIN commission c ON c.id = p.commission_id
JOIN node n ON n.id = c.node_id
WHERE p.user_id = $1
ORDER BY c.id
