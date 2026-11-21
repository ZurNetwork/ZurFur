-- Store one Slot's satellite row, keyed by its carrying element's id and bound to its
-- node, which holds the Slot's name. The node must be a `slot` node whose segment is
-- the element's id, under the `slots` entry of this commission's own node; any other
-- node inserts no row. No occupant column of any kind: fill is the Character epic's.
-- $1: element_id
-- $2: commission_id
-- $3: node_id
-- $4: notes
INSERT INTO commission_slot (element_id, commission_id, notes, node_id)
SELECT given.element_id, given.commission_id, given.notes, n.id
FROM (VALUES (
    $1::uuid, $2::uuid, $3::uuid,
    -- COALESCE marks the nullable notes as an optional parameter.
    COALESCE($4::text)
)) AS given (element_id, commission_id, node_id, notes)
JOIN node n ON n.id = given.node_id
JOIN node slots ON slots.id = n.parent
JOIN commission c ON c.node_id = slots.parent
WHERE n.type = 'slot'
  AND n.segment = given.element_id::text
  AND slots.type = 'commission.slots'
  AND c.id = given.commission_id
