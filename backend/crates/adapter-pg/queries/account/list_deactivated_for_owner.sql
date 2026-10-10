-- Every soft-deleted (deactivated) account in which `$1` holds the Owner
-- role. The one read that returns deactivated rows, and only to their Owner:
-- `find` and `list_for_user` keep filtering on `deleted_at IS NULL`.
--
-- ORDER BY … COLLATE "C" sorts the DID by byte value, as the adapter-mem twin
-- sorts in process.
SELECT a.id, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at
FROM account_members am
JOIN accounts a ON a.id = am.account_id
WHERE am.user_id = $1
  AND am.role = $2
  AND a.deleted_at IS NOT NULL
ORDER BY a.id COLLATE "C"
