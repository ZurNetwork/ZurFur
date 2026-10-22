-- One commission's summary: its node's name and level, and whether it is
-- archived, and nothing else of the row. The node is the one the commission
-- points at; a commission with no node reads as absent. The archive time itself
-- is never decoded. Archived commissions are found.
SELECT n.name, n.level, c.archived_at IS NOT NULL AS archived
FROM commission c
JOIN node n ON n.id = c.node_id
WHERE c.id = $1
