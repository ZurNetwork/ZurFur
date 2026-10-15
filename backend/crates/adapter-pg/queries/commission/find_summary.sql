-- One commission's summary: its title as stored, its own visibility and
-- whether it is archived, and nothing else of the row. The archive time itself
-- is never decoded. Archived commissions are found.
SELECT title, visibility, archived_at IS NOT NULL AS archived
FROM commission
WHERE id = $1
