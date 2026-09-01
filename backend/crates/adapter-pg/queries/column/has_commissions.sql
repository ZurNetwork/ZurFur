-- Whether a column still holds any card — the gate on deleting one, so removing
-- a list never silently drops the cards on it.
SELECT EXISTS (
    SELECT 1 FROM workflow_column_commission WHERE column_id = $1
) AS "exists!"
