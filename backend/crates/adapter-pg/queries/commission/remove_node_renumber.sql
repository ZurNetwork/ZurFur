UPDATE commission_node AS node
SET position = renumbered.position
FROM (
    SELECT id, (ROW_NUMBER() OVER (ORDER BY position))::int - 1 AS position
    FROM commission_node
    WHERE parent = $1 AND commission_id = $2
) AS renumbered
WHERE node.id = renumbered.id AND node.position <> renumbered.position
