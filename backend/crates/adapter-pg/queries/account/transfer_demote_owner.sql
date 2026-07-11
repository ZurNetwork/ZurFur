UPDATE account_members
SET "role" = $1, parent = $4
WHERE account_id = $2 AND user_id = $3 AND "role" = $5
