SELECT level
FROM commission_view_grant
WHERE commission_id = $1 AND account_id = $2
