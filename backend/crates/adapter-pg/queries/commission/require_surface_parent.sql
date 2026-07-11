-- params: parent, commission_id
-- fetch: optional
-- row: SurfaceParentRow
SELECT type AS type_tag, mode FROM commission_node
WHERE id = $1 AND commission_id = $2
FOR UPDATE
