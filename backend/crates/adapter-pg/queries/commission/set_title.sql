UPDATE commission
SET title = $2
WHERE id = $1 AND title IS DISTINCT FROM $2
