UPDATE commission
SET linked_channel = $2
WHERE id = $1 AND linked_channel IS DISTINCT FROM $2
