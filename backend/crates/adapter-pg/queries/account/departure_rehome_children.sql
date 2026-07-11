-- params: parent?, account_id, user_id
-- fetch: execute
UPDATE account_members SET parent = $1 WHERE account_id = $2 AND parent = $3
