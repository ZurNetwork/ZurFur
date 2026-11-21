-- Move a commission's lifecycle step by compare-and-set: the row changes only while it still
-- holds the step the move was checked from, so a stale move matches no row.
-- $2: to_step
-- $3: from_step
UPDATE commission
SET lifecycle = moved.to_step
FROM (VALUES ($2::text, $3::text)) AS moved (to_step, from_step)
WHERE commission.id = $1 AND commission.lifecycle = moved.from_step
