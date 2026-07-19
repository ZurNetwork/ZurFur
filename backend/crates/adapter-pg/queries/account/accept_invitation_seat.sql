-- Seat the invited User at the offered role. If the pair is already seated (a
-- role granted through another path, e.g. `grant_role`, while this invitation
-- sat pending), `ON CONFLICT DO NOTHING` skips the insert and leaves the
-- existing row untouched rather than raising a primary-key violation; RETURNING
-- then yields no row, and the caller (`PgAccountWrites::accept_invitation`)
-- falls back to reading the already-persisted role.
INSERT INTO account_members (account_id, user_id, parent, "role", listed_on_profile)
VALUES ($1, $2, $3, $4, $5)
ON CONFLICT (account_id, user_id) DO NOTHING
RETURNING account_id, user_id, "role"
