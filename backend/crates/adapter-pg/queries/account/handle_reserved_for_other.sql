-- changed_at receives the reservation-window start (now - quarantine window);
-- account_id optionally excludes the renaming account itself.
SELECT EXISTS (
    SELECT 1
    FROM account_handle_changes
    WHERE old_handle = $1
      AND changed_at >= $2
      AND ($3::uuid IS NULL OR account_id <> $3)
) AS reserved