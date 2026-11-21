-- Set or clear a commission's deadline by compare-and-set: the row changes only while it still
-- holds the deadline the change was checked from, so a stale change matches no row.
UPDATE commission AS held
SET deadline = $2
WHERE held.id = $1 AND held.deadline IS NOT DISTINCT FROM $3
