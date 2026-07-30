SELECT type AS type_tag, mode, depth FROM commission_node
WHERE id = $1 AND commission_id = $2
FOR UPDATE
