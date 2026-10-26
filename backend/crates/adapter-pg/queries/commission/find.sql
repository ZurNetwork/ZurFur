SELECT owner_id, lifecycle, disputed_from, disputed_at, deadline, maturity, graphic,
       direction_status, deadline_status, linked_channel, archived_at, created_at
FROM commission
WHERE id = $1
