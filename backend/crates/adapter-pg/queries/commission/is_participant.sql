-- params: commission_id, user_id
-- fetch: one
-- not_null: is_participant
SELECT EXISTS(
    SELECT 1 FROM commission WHERE id = $1 AND owner_id = $2
) AS is_participant
