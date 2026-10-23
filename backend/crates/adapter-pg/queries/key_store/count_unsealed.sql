-- How many rows have no sealed envelope yet.
SELECT count(*)
FROM account_keys
WHERE sealed_keys IS NULL
