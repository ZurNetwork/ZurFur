-- The lone pending offer for (commission, seat, invited_user), or nothing
-- (ZMVP-78). The `state` bind is the pending discriminant; accepted/revoked
-- invitations are history, not live offers, so they never match. Scoped to
-- commission_id in the query itself so a seat id from another commission's
-- tree can never reach that commission's offers — the authorization binding
-- is unrepresentable to skip, not caller discipline. The Seat mirror of
-- `account/find_pending_invitation.sql`.
SELECT id, commission_id, seat_id, invited_user, inviter, state, created_at, updated_at
FROM commission_invitation
WHERE commission_id = $1 AND seat_id = $2 AND invited_user = $3 AND state = $4
LIMIT 1
