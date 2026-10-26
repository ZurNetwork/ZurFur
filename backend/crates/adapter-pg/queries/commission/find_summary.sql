-- One commission's summary as one viewer reads it, always as exactly one row.
-- A materialized one-row probe is left-joined to `commission` and then to
-- `node`, so an absent id and a present one probe the node index once each and
-- run the participant probe exactly once. An absent id, or a commission with no
-- node, probes the node index with the nil uuid, which no node is minted with.
-- The row is shown when the commission exists with its node and the viewer owns
-- it, takes part in it, or it is live and Listed or Public. A row not shown is
-- the absent id's: `found` false and every other column NULL, its name never
-- read. A shown row holds the node's name and level, the viewer's ownership and
-- participation, and whether the commission is archived; the archive time
-- itself is never decoded.
-- $1: commission_id
-- $2: viewer
WITH probed AS MATERIALIZED (
    SELECT COALESCE(node.id = commission.node_id, false) AS found,
           node.name,
           node.level,
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
), shown AS (
    SELECT probed.*,
           probed.found AND (
               COALESCE(probed.is_owner, false)
               OR probed.is_participant
               OR (NOT probed.archived AND probed.level IN ('listed', 'public'))
           ) AS shown
    FROM probed
)
SELECT shown.shown AS found,
       CASE WHEN shown.shown THEN shown.name END AS name,
       CASE WHEN shown.shown THEN shown.level END AS level,
       CASE WHEN shown.shown THEN shown.is_owner END AS is_owner,
       CASE WHEN shown.shown THEN shown.is_participant END AS is_participant,
       CASE WHEN shown.shown THEN shown.archived END AS archived
FROM shown
