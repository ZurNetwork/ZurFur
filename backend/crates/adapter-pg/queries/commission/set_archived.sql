UPDATE commission
SET archived_at = $2
WHERE id = $1 AND (archived_at IS NULL) <> ($2::timestamptz IS NULL)
