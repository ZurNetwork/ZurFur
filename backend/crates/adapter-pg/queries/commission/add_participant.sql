-- Insert a participant membership row (ZMVP-76). `(commission_id, user_id)`
-- is the table's PRIMARY KEY, so a re-add for an already-seated pair is a
-- silent no-op rather than a constraint violation: the owner's row is born
-- here (CommissionWrites::create) and ZMVP-79's seat acceptance re-adds
-- whoever it seats, who may already be a participant through another seat (a
-- User can hold multiple seats, Engineer ruling 2026-07-16) — the invariant
-- of at most one membership row per pair is unreachable to violate rather
-- than caller discipline. The original `created_at` is preserved.
INSERT INTO commission_participant (commission_id, user_id, created_at)
VALUES ($1, $2, $3)
ON CONFLICT (commission_id, user_id)
DO NOTHING
