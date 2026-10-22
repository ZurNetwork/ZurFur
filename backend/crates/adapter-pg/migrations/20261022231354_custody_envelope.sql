-- Adds `sealed_keys`, an Account's keys sealed in the versioned custody
-- envelope, beside the 0.1 ciphertext in `wrapped_keys`, which is kept as it is
-- and may now be NULL. Every row holds at least one of the two. Why: this
-- crate's NODE.json notes, under this file.
ALTER TABLE account_keys
    ADD COLUMN sealed_keys bytea,
    ALTER COLUMN wrapped_keys DROP NOT NULL,
    ADD CONSTRAINT account_keys_holds_keys
        CHECK (wrapped_keys IS NOT NULL OR sealed_keys IS NOT NULL);
