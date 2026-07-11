-- params: account_id, user_id, role
-- fetch: execute
INSERT INTO account_members (account_id, user_id, role)
VALUES ($1, $2, $3)
