-- params: account_id, user_id, parent, role, listed_on_profile
-- fetch: one
-- row: SeatedMemberRow
INSERT INTO account_members (account_id, user_id, parent, "role", listed_on_profile)
VALUES ($1, $2, $3, $4, $5)
RETURNING account_id, user_id, "role"
