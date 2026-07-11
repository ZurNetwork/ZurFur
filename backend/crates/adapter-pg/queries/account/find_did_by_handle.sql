-- params: handle
-- fetch: optional
SELECT did FROM accounts WHERE handle = $1 AND deleted_at IS NULL
