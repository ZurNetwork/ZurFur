-- The signed-in user's OWNED commissions, owner-POV only. Archived
-- commissions are excluded — an active-view listing, per the documented
-- listing-projection contract on `commission.archived_at`. Ordered by id
-- (UUIDv7 sorts as creation order).
SELECT id, owner_id, lifecycle, disputed_from, disputed_at, deadline, maturity, graphic,
       direction_status, deadline_status, linked_channel, archived_at, created_at
FROM commission
WHERE owner_id = $1
  AND archived_at IS NULL
ORDER BY id
