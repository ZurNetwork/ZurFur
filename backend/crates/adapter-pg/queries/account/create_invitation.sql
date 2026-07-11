-- params: id, account_id, invited_user, role, inviter, state, created_at, updated_at
-- fetch: execute
INSERT INTO account_invitations
    (id, account_id, invited_user, role, inviter, state, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (account_id, invited_user) WHERE state = 'pending'
DO NOTHING
