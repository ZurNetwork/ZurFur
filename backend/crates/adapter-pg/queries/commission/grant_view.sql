INSERT INTO commission_view_grant (commission_id, account_id, level)
VALUES ($1, $2, $3)
ON CONFLICT (commission_id, account_id)
DO UPDATE SET level = EXCLUDED.level
