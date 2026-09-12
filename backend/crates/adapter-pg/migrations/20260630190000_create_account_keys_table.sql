-- Custody store for the private keys behind a minted account did:plc.
-- Every key is envelope-encrypted under a root key held outside the
-- database before it is written — see NODE.md
-- ("20260630190000_create_account_keys_table.sql") for the full rationale.
--
-- did           The account's did:plc — the natural, unique key. There is
--               deliberately NO foreign key to accounts(did): the keys are written
--               during minting, BEFORE the account row exists (the DID is derived
--               from the very operation these keys sign), so a FK would fail. One
--               DID mints once, so the PRIMARY KEY also enforces "custody written
--               at most once".
-- wrapped_keys  The AEAD ciphertext: a random per-row nonce followed by the sealed
--               bundle of the three 32-byte secp256k1 private scalars. Opaque
--               bytes; only a holder of the root key can open it. Never contains
--               plaintext key material.
-- key_version   The envelope scheme / root-key generation, so keys can be re-wrapped
--               under a new root key (or KMS) later without a data migration guess.
-- created_at    When custody was taken. Application-supplied (no DEFAULT now()),
--               matching the codebase convention.
CREATE TABLE account_keys (
    did          text        PRIMARY KEY,
    wrapped_keys bytea       NOT NULL,
    key_version  integer     NOT NULL DEFAULT 1,
    created_at   timestamptz NOT NULL
);
