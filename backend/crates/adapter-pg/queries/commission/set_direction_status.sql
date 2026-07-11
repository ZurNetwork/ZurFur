UPDATE commission
SET direction_status = $2
WHERE id = $1 AND direction_status IS DISTINCT FROM $2
