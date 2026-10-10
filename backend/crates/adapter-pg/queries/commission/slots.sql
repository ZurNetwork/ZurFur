-- Every Slot declared on one commission, in declaration order (the carrying
-- element ids are UUIDv7). Empty for a commission with none and for an
-- unknown one.
SELECT element_id, commission_id, title, notes
FROM commission_slot
WHERE commission_id = $1
ORDER BY element_id
