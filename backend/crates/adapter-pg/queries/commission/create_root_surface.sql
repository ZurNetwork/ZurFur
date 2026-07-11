-- params: id, commission_id, mode, created_by, created_at
-- fetch: execute
INSERT INTO commission_node
    (id, commission_id, parent, type, mode, position, created_by, created_at)
VALUES ($1, $2, NULL, 'surface', $3, 0, $4, $5)
