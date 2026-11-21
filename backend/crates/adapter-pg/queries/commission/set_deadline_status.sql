-- Raise or take down a commission's Delayed flag by compare-and-set: the row changes only while
-- it still holds the stored flag the change was checked from, so a stale change matches no row.
UPDATE commission AS held
SET deadline_status = $2
WHERE held.id = $1 AND held.deadline_status IS NOT DISTINCT FROM $3
