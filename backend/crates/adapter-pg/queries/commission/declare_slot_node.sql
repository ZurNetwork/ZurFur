INSERT INTO commission_node
    (id, commission_id, parent, type, mode, position, created_by, created_at)
VALUES (
    $1, $2, $3, 'component', NULL,
    (SELECT COALESCE(MAX(position) + 1, 0) FROM commission_node WHERE parent = $3),
    $4, $5
)
