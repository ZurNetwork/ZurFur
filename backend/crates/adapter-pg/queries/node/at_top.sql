-- The top-level node of one type holding one segment, if any. The type is
-- matched too, so a top-level node of another type is never taken for it.
SELECT id, parent, segment, type, level, name
FROM node
WHERE parent IS NULL AND type = $1 AND segment = $2
