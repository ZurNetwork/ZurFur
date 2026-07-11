-- params: owner_role, account_id, new_owner
-- fetch: execute
UPDATE account_members
SET "role" = $1, parent = NULL
WHERE account_id = $2 AND user_id = $3
