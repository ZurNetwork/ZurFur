-- params: id, commission_id, parent, mode, created_by, created_at
-- fetch: execute
INSERT INTO commission_node
    (id, commission_id, parent, type, mode, position, created_by, created_at)
VALUES (
    $1, $2, $3, 'surface', $4,
    (SELECT COALESCE(MAX(position) + 1, 0) FROM commission_node WHERE parent = $3),
    $5, $6
)
