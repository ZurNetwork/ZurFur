-- The system's own act's lock, with no viewer: the node the commission points
-- at and that node's level, with the commission row locked until the unit of
-- work ends. FOR NO KEY UPDATE OF commission locks the commission row alone, as
-- the standing read does, so a user's act on the same commission waits for this
-- unit, and this one for it. An absent id, or a commission with no node,
-- answers no row.
SELECT node.id AS node_id,
       node.level
FROM commission
JOIN node ON node.id = commission.node_id
WHERE commission.id = $1
FOR NO KEY UPDATE OF commission
