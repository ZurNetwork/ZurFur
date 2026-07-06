-- params: commission_id
-- fetch: many
-- row: SeatRow
SELECT id, kind, prompt, link, occupant
FROM commission_seat
WHERE commission_id = $1
ORDER BY id
