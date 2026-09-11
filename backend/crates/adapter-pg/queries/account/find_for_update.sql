-- `find`, with the row locked until the unit of work commits. FOR NO KEY UPDATE is
-- the weaker of the two row locks: it blocks a concurrent writer of THIS account
-- while still letting other transactions insert children that reference it (a
-- membership, an invitation), which a plain FOR UPDATE would stall.
SELECT a.id, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at
FROM accounts a
WHERE a.id = $1
  AND a.deleted_at IS NULL
FOR NO KEY UPDATE
