-- params: id, commission_id, uploaded_by, created_at
-- fetch: execute
INSERT INTO commission_file (id, commission_id, uploaded_by, created_at)
VALUES ($1, $2, $3, $4)
