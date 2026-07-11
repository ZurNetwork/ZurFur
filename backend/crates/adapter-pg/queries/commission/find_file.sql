-- params: key, commission_id
-- fetch: optional
-- row: CommissionFileRow
SELECT id, commission_id, uploaded_by, created_at
FROM commission_file
WHERE id = $1 AND commission_id = $2
