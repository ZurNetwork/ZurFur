-- Every row with no sealed envelope yet, by DID, locked until the unit of work
-- ends. Such a row always holds its 0.1 ciphertext.
SELECT did, wrapped_keys
FROM account_keys
WHERE sealed_keys IS NULL
  AND wrapped_keys IS NOT NULL
ORDER BY did
FOR UPDATE
