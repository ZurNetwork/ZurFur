SELECT seq, account_id, placed_by, placed_at
FROM commission_placement
WHERE commission_id = $1
ORDER BY seq
