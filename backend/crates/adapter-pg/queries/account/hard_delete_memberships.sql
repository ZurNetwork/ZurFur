-- params: account_id
-- fetch: execute
DELETE FROM account_members WHERE account_id = $1
