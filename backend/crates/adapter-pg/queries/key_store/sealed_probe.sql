-- Every row holding a sealed envelope, by DID, so the probe can open each one.
SELECT did, sealed_keys
FROM account_keys
WHERE sealed_keys IS NOT NULL
ORDER BY did
