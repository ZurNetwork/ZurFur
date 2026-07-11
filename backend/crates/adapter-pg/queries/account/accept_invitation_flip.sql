-- params: accepted_state, updated_at, id, pending_state
-- fetch: execute
UPDATE account_invitations
SET state = $1, updated_at = $2
WHERE id = $3 AND state = $4
