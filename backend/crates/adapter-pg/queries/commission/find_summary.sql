-- One commission's summary, always as exactly one row: a one-row probe
-- left-joined to `commission` and then to `node`, so an absent id and a present
-- one probe the node index once each. An absent id, or a commission with no
-- node, probes it with the nil uuid, which no node is minted with. `found` is
-- true only when the commission exists and the joined node is the one it points
-- at; then the row holds that node's name and level and whether the commission
-- is archived, and nothing else of the row. The archive time itself is never
-- decoded. Archived commissions are found.
-- $1: commission_id
SELECT COALESCE(node.id = commission.node_id, false) AS found,
       node.name,
       node.level,
       commission.archived_at IS NOT NULL AS archived
FROM (VALUES ($1::uuid)) AS probe (commission_id)
LEFT JOIN commission ON commission.id = probe.commission_id
LEFT JOIN node
    ON node.id = COALESCE(commission.node_id, '00000000-0000-0000-0000-000000000000'::uuid)
