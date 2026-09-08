-- Resolve a DID to its User without minting one. Since the actor re-key
-- (DD 57081857) a User's id IS its DID, so this and `find` address the same column —
-- they stay two statements because they are two port methods: `find` takes a
-- `UserId` a session already holds, this takes a `Did` handed over by the PDS.
SELECT u.id, u.created_at
FROM users u
WHERE u.id = $1
