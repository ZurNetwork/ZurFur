-- Revoke one view grant — a hard delete, so the key is gone on the next render.
-- Removing a key nobody holds
-- matches nothing, which is how the caller learns there was no transition.
DELETE FROM commission_view_grant
WHERE commission_id = $1 AND grantee = $2
