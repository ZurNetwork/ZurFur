-- A column's cards in board order. The card order is a plain integer index,
-- rewritten wholesale by `set_commissions` — the domain's `Column.commissions`
-- is an ordered Vec with no per-card key, so there is no insert-between here.
SELECT commission_id
FROM workflow_column_commission
WHERE column_id = $1
ORDER BY position
