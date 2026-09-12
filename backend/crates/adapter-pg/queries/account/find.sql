-- One live account by id. `accounts.id` IS the account's DID, so the row
-- carries no separate `did` column and needs no actor_identity join to
-- recover one.
SELECT a.id, a.handle, a.name, a.created_at, a.updated_at, a.deleted_at
FROM accounts a
WHERE a.id = $1
  AND a.deleted_at IS NULL
