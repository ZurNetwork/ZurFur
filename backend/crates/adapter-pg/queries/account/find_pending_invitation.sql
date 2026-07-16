SELECT id, account_id, invited_user, role, inviter, state, created_at, updated_at
FROM account_invitations
WHERE account_id = $1 AND invited_user = $2 AND state = $3
LIMIT 1
