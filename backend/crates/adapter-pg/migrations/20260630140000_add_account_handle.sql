-- Adds the account's public handle: required, globally unique, chosen at
-- founding time. See NODE.md ("20260630140000_add_account_handle.sql") for
-- the full rationale.
ALTER TABLE accounts ADD COLUMN handle text NOT NULL;

CREATE UNIQUE INDEX accounts_handle_key ON accounts (handle);
