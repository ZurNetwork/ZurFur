-- The first row holding a sealed envelope, by DID: the one row a command
-- opens to check its root. Read only once `sealed_keys` exists.
SELECT did, sealed_keys
FROM account_keys
WHERE sealed_keys IS NOT NULL
ORDER BY did
LIMIT 1
