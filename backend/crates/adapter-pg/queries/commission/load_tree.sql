-- params: commission_id
-- fetch: many
-- row: TreeNodeRow
SELECT id, parent, type AS type_tag, mode, position, created_by, created_at, payload
FROM commission_node
WHERE commission_id = $1
