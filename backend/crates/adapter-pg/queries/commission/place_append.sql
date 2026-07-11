INSERT INTO commission_placement (commission_id, account_id, placed_by, placed_at)
VALUES ($1, $2, $3, $4)
RETURNING seq
