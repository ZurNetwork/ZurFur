-- params: node, commission_id
-- fetch: execute
DELETE FROM commission_node WHERE id = $1 AND commission_id = $2
