-- Rename the commission's node, the one the commission points at, where its
-- name differs; the node's trigger copies the name onto the commission's title.
UPDATE node n
SET name = $2
FROM commission c
WHERE c.id = $1
  AND n.id = c.node_id
  AND n.name IS DISTINCT FROM $2
