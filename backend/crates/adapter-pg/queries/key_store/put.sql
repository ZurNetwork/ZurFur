-- Store a newly minted DID's sealed envelope. Writes no 0.1 ciphertext;
-- key_version keeps its default.
INSERT INTO account_keys (did, sealed_keys, created_at)
VALUES ($1, $2, $3)
