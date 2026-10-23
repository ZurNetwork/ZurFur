-- Every row still holding a 0.1 ciphertext, by DID. Names only columns the
-- table had before the custody envelope, so it also runs on that schema.
SELECT did, wrapped_keys
FROM account_keys
WHERE wrapped_keys IS NOT NULL
ORDER BY did
