-- state receives the revoked state; grantable lists the roles the issuer's new role may
-- still offer (an empty list revokes every pending offer they issued); inv_state selects
-- the still-pending offers. Sets only state and updated_at.
UPDATE account_invitations AS inv SET state = $1, updated_at = $2
WHERE inv.account_id = $3 AND inv.inviter = $4 AND inv.state = $5
  AND NOT (inv.role = ANY($6::text[]))
