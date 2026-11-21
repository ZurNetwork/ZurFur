-- A row's sealed envelope, by DID: NULL until the row is re-wrapped. Never
-- reads the 0.1 ciphertext.
SELECT sealed_keys FROM account_keys WHERE did = $1
