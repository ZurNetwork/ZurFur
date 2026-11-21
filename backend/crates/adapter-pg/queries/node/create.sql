-- Store one node. The node table's checks and triggers refuse a node that
-- breaks its shape or doesn't fit its parent.
INSERT INTO node (id, parent, segment, type, level, name)
VALUES ($1, $2, $3, $4, $5, $6)
