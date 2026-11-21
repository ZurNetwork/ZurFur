-- `role_of`, with the membership row locked FOR SHARE until the unit commits: a
-- change of that member's role waits, while other share holders proceed.
SELECT role
FROM account_members
WHERE user_id = $1
  AND account_id = $2
LIMIT 1
FOR SHARE
