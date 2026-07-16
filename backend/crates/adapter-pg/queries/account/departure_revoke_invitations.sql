-- state receives the revoked state; inv_state selects the departing inviter's
-- still-pending invitations.
UPDATE account_invitations AS inv SET state = $1, updated_at = $2
WHERE inv.account_id = $3 AND inv.inviter = $4 AND inv.state = $5
