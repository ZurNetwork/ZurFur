-- `find`, with the row locked FOR SHARE until the unit of work commits: writers of
-- the account's members and invitations, which lock it the same way, proceed side by
-- side, while an update or a delete of the account row waits for them all.
SELECT a.id, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at
FROM accounts a
WHERE a.id = $1
  AND a.deleted_at IS NULL
FOR SHARE
