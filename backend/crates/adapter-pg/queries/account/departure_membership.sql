-- The departing member's role and parent, with their row locked until the unit
-- commits, so a concurrent ownership transfer either commits first or waits.
SELECT m."role", m.parent FROM account_members AS m
WHERE m.account_id = $1 AND m.user_id = $2
FOR UPDATE
