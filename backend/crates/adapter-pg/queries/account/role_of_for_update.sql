-- `role_of`, with the membership row locked FOR UPDATE until the unit commits: any
-- other write or lock of that row waits.
SELECT role
FROM account_members
WHERE user_id = $1
  AND account_id = $2
LIMIT 1
FOR UPDATE
