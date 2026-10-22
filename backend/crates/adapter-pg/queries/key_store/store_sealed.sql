-- Store a row's sealed envelope, only while it has none: the envelope is
-- written once. Changes one row, or none when the row is missing or sealed.
UPDATE account_keys
SET sealed_keys = $2
WHERE did = $1
  AND sealed_keys IS NULL
