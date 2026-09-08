-- The level one grantee holds on one commission, or nothing. Addressed by the
-- holder's DID (the actor re-key, DD 57081857), so the same statement answers for
-- whichever actor class the caller is asking about.
SELECT level
FROM commission_view_grant
WHERE commission_id = $1 AND grantee = $2
