SELECT seq, account_id, placed_by, placed_at
FROM commission_current_placement
WHERE commission_id = $1
