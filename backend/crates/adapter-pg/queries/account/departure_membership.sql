-- params: account_id, user_id
-- fetch: optional
SELECT parent FROM account_members WHERE account_id = $1 AND user_id = $2
