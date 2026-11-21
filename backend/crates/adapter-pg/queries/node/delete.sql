-- Delete one node; the parent key's cascade takes every node under it. A
-- fixed entry is refused while its commission node lives.
DELETE FROM node
WHERE id = $1
