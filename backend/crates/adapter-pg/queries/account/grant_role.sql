-- role receives the member's new role; m_role guards that the member is not
-- the owner. An update, never an insert: a pair with no membership matches no
-- row, so a grant seats no one.
UPDATE account_members AS m
SET "role" = $3
WHERE m.account_id = $1 AND m.user_id = $2 AND m."role" <> $4
