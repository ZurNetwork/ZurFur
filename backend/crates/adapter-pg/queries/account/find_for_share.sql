-- `find`, with the row locked FOR SHARE until the unit of work commits: an update or
-- a delete of the account row waits until then, while other share holders proceed.
SELECT a.id, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at
FROM accounts a
WHERE a.id = $1
  AND a.deleted_at IS NULL
FOR SHARE
