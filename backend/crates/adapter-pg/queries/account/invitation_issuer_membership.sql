-- The issuer's membership, with their row key-share locked until the unit commits,
-- so a concurrent departure of the issuer either commits first or waits.
SELECT m.user_id FROM account_members AS m
WHERE m.account_id = $1 AND m.user_id = $2
FOR KEY SHARE
