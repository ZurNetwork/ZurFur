-- Store a new commission bound to its node, taking its title and visibility from
-- that node. The node must be a top-level `commission` node whose segment is the
-- commission's id; any other node inserts no row.
-- $1: id
-- $2: node_id
-- $3: owner_id
-- $4: lifecycle
-- $5: deadline
-- $6: maturity
-- $7: graphic
-- $8: created_at
INSERT INTO commission
    (id, node_id, title, owner_id, lifecycle, visibility, deadline, maturity, graphic, created_at)
SELECT given.id, n.id, n.name, given.owner_id, given.lifecycle, n.level,
       given.deadline, given.maturity, given.graphic, given.created_at
FROM (VALUES (
    $1::uuid, $2::uuid, $3::text, $4::text,
    -- COALESCE marks the three nullable values as optional parameters.
    COALESCE($5::timestamptz), COALESCE($6::text), COALESCE($7::boolean),
    $8::timestamptz
)) AS given (id, node_id, owner_id, lifecycle, deadline, maturity, graphic, created_at)
JOIN node n ON n.id = given.node_id
WHERE n.type = 'commission'
  AND n.parent IS NULL
  AND n.segment = given.id::text
