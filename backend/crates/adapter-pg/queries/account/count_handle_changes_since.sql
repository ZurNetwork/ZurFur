-- changed_at receives the rate-limit window start (now - window).
SELECT count(*)
FROM account_handle_changes
WHERE account_id = $1 AND changed_at >= $2