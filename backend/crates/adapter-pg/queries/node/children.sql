-- Every node directly under one parent, by segment bytewise (the column is
-- COLLATE "C").
SELECT id, parent, segment, type, level, name
FROM node
WHERE parent = $1
ORDER BY segment
