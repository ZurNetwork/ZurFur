-- state receives the accepted state; inv_state guards the expected current
-- (pending) state, so a concurrent flip loses.
UPDATE account_invitations AS inv
SET state = $1, updated_at = $2
WHERE inv.id = $3 AND inv.state = $4
