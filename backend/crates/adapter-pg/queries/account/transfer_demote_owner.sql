-- params: admin_role, account_id, old_owner, new_owner, owner_role
-- fetch: execute
UPDATE account_members
SET "role" = $1, parent = $4
WHERE account_id = $2 AND user_id = $3 AND "role" = $5
