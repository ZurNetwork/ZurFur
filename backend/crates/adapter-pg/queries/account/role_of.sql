SELECT role
FROM account_members
WHERE user_id = $1
  AND account_id = $2
LIMIT 1
