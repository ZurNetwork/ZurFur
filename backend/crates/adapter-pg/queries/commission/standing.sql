-- One viewer's standing on one commission, always as exactly one row: a one-row
-- probe left-joined to `commission` and then to `node`, so an absent id and a
-- stranger run the same statement, get the same row shape, run the same
-- participant probe and probe the node index once each. An absent id, or a
-- commission with no node, probes it with the nil uuid, which no node is minted
-- with. `found` is false for an absent id; `level` is the commission's own
-- node's, NULL when there is none. Reads no name and no whole row, and never
-- decodes the archive time, only whether there is one.
-- $1: commission_id
-- $2: viewer
SELECT commission.id IS NOT NULL AS found,
       CASE WHEN node.id = commission.node_id THEN node.level END AS level,
       commission.owner_id = probe.viewer AS is_owner,
       EXISTS(
           SELECT 1 FROM commission_participant
           WHERE commission_participant.commission_id = probe.commission_id
             AND commission_participant.user_id = probe.viewer
       ) AS is_participant,
       commission.archived_at IS NOT NULL AS archived
FROM (VALUES ($1::uuid, $2::text)) AS probe (commission_id, viewer)
LEFT JOIN commission ON commission.id = probe.commission_id
LEFT JOIN node
    ON node.id = COALESCE(commission.node_id, '00000000-0000-0000-0000-000000000000'::uuid)
