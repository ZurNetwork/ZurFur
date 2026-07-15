-- role receives the demoted (admin) role and parent the new owner to re-home
-- under; m_role guards that the demoted member still holds the owner role.
UPDATE account_members AS m
SET "role" = $1, parent = $4
WHERE m.account_id = $2 AND m.user_id = $3 AND m."role" = $5
