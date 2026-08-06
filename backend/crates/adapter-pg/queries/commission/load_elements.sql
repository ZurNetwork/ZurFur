-- Every element of one commission — the element third of the whole-composition
-- read (ZMVP-166), one indexed query on commission_id. Ordered by the addressing
-- tuple so the ordering the store assigns is the ordering a caller observes,
-- without a sort in Rust.
SELECT id, tab_id, surface, type AS element_type, mode, band, position,
       created_by, created_at, payload
FROM commission_element
WHERE commission_id = $1
ORDER BY tab_id, surface, band, position
