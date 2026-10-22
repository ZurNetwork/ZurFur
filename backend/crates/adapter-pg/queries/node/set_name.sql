-- Replace one node's name; no row when there is no such node.
UPDATE node
SET name = $2
WHERE id = $1
