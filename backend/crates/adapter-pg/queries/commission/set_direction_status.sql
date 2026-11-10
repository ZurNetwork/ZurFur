-- Set or clear a commission's direction status by compare-and-set: the row changes only while
-- it still holds the status the change was checked from, so a stale change matches no row.
UPDATE commission AS held
SET direction_status = $2
WHERE held.id = $1 AND held.direction_status IS NOT DISTINCT FROM $3
