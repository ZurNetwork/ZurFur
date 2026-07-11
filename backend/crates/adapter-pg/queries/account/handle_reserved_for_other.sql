SELECT EXISTS (
    SELECT 1
    FROM account_handle_changes
    WHERE old_handle = $1
      AND changed_at >= $2
      AND ($3::uuid IS NULL OR account_id <> $3)
) AS reserved
