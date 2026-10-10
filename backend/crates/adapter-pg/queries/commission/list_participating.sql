-- The commissions a User is a Participant of, read from the same
-- `commission_participant` record `is_participant` reads. Archived commissions
-- are included; hiding them is the caller's. Seeks the by-user index; ordered
-- by commission id (UUIDv7 sorts as creation order).
SELECT c.id, c.title, c.visibility, c.archived_at
FROM commission_participant p
JOIN commission c ON c.id = p.commission_id
WHERE p.user_id = $1
ORDER BY c.id
