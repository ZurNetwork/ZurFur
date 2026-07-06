-- params: commission_id, user_id, created_at
-- fetch: execute
INSERT INTO commission_participant (commission_id, user_id, created_at)
VALUES ($1, $2, $3)
