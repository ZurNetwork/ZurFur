SELECT EXISTS(
    SELECT 1 FROM commission WHERE id = $1 AND owner_id = $2
) AS is_participant
