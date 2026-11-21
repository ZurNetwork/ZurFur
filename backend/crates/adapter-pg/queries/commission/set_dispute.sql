-- Put a commission into Disputed by compare-and-set, storing the step it left and when the
-- dispute began: the row changes only while it still holds the step the raise was checked
-- from, so a stale raise matches no row.
-- $2: from_step
-- $3: began_at
UPDATE commission
SET lifecycle = 'disputed', disputed_from = raised.from_step, disputed_at = raised.began_at
FROM (VALUES ($2::text, $3::timestamptz)) AS raised (from_step, began_at)
WHERE commission.id = $1 AND commission.lifecycle = raised.from_step
