-- params: commission_id, user_id
-- fetch: one
-- not_null: is_participant
SELECT EXISTS(
    SELECT 1 FROM commission_participant
    WHERE commission_id = $1 AND user_id = $2
) AS is_participant
