-- Return a Disputed commission to the step its dispute left, read from the row, and forget the
-- dispute, by compare-and-set: the row changes only while it is still Disputed from the step
-- the clear was checked against, so a stale clear matches no row.
-- $2: to_step
UPDATE commission
SET lifecycle = commission.disputed_from, disputed_from = NULL, disputed_at = NULL
FROM (VALUES ($2::text)) AS cleared (to_step)
WHERE commission.id = $1
  AND commission.lifecycle = 'disputed'
  AND commission.disputed_from = cleared.to_step
