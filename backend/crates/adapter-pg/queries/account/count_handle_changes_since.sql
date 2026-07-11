-- params: account_id, since
-- fetch: one
-- not_null: count
SELECT count(*)
FROM account_handle_changes
WHERE account_id = $1 AND changed_at >= $2
