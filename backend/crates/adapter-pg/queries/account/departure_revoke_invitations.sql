UPDATE account_invitations SET state = $1, updated_at = $2
WHERE account_id = $3 AND inviter = $4 AND state = $5
