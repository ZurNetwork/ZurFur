-- Persist a freshly issued, pending seat invitation (ZMVP-78). The partial
-- unique index (`... WHERE state = 'pending'`, see the migration) enforces at
-- most one pending offer per (seat, invited_user), so a duplicate issue is
-- silently dropped rather than becoming a second row — the store-level backstop
-- for the idempotent re-invite the handler also guards. The Seat mirror of
-- `account/create_invitation.sql`.
INSERT INTO commission_invitation
    (id, commission_id, seat_id, invited_user, inviter, state, created_at, updated_at)
VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
ON CONFLICT (seat_id, invited_user) WHERE state = 'pending'
DO NOTHING
