-- The node under one parent holding one segment, if any.
SELECT id, parent, segment, type, level, name
FROM node
WHERE parent = $1 AND segment = $2
