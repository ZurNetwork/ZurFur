SELECT id, kind, prompt, link, occupant
FROM commission_seat
WHERE commission_id = $1
ORDER BY id
