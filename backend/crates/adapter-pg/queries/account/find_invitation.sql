-- params: id
-- fetch: optional
-- row: InvitationRow
SELECT id, account_id, invited_user, role, inviter, state, created_at, updated_at
FROM account_invitations
WHERE id = $1
LIMIT 1
