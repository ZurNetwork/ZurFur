-- params: id
-- fetch: optional
-- row: CommissionRow
SELECT title, owner_id, lifecycle, visibility, deadline, maturity, graphic,
       direction_status, deadline_status, linked_channel, archived_at, created_at
FROM commission
WHERE id = $1
