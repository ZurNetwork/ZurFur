-- params: account_id, user_id
-- fetch: execute
DELETE FROM account_members WHERE account_id = $1 AND user_id = $2
