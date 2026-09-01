-- A card's index within one column, or nothing if that column does not hold it.
SELECT position
FROM workflow_column_commission
WHERE column_id = $1 AND commission_id = $2
