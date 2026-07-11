-- params: node, commission_id
-- fetch: optional
SELECT parent FROM commission_node WHERE id = $1 AND commission_id = $2
