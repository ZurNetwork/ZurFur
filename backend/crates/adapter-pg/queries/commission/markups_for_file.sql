-- Every markup on one file entry, in draw order — the UUIDv7 id sorts as creation
-- order, so no separate ordering column is carried. Scoped by commission_id: a
-- file key from a different commission matches nothing and answers with an empty
-- set rather than a signal (the non-oracle rule find_file follows).
SELECT id, commission_id, file_id, added_by, shape, text, created_at
FROM commission_markup
WHERE commission_id = $1 AND file_id = $2
ORDER BY id
