-- Every Slot declared on one commission, in declaration order (the carrying
-- element ids are UUIDv7), each titled by its node's name. Empty for a
-- commission with none and for an unknown one.
SELECT s.element_id, s.commission_id, n.name AS title, s.notes
FROM commission_slot s
JOIN node n ON n.id = s.node_id
WHERE s.commission_id = $1
ORDER BY s.element_id
