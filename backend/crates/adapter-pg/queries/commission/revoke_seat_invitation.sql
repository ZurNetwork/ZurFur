-- Revoke a pending seat invitation (ZMVP-78). state receives the revoked state;
-- inv_state guards the expected current (pending) state, so a concurrent flip
-- loses, and an UPDATE matching no row still succeeds — revoking an absent or
-- already-terminal invitation is a harmless no-op. The Seat mirror of
-- `account/revoke_invitation.sql`.
UPDATE commission_invitation AS inv
SET state = $1, updated_at = $2
WHERE inv.id = $3 AND inv.state = $4
