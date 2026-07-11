-- params: account_id, user_id, role
-- fetch: execute
INSERT INTO account_members (account_id, user_id, role)
VALUES ($1, $2, $3)
ON CONFLICT (account_id, user_id) DO UPDATE
    SET role = EXCLUDED.role
