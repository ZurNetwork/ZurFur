-- Add the Account's public handle (ZMVP-44, DD "The Account Handle" 24870914 §6).
--
-- handle  The validated, normalized atproto handle the account is reached by,
--         chosen by the founder at POST /accounts. Either a Zurfur-issued
--         '<label>.zurfur.app' subdomain or a brought (BYO) domain; both are
--         validated + normalized by the domain `Handle` newtype (lowercase, no
--         trailing dot, punycode/reserved-label rejects) before they reach here.
--         REQUIRED and UNIQUE across live accounts: one handle, one account.
--         The stored value is the whole handle (e.g. 'alice.zurfur.app'), so the
--         '/.well-known/atproto-did' resolver looks up by exact match on the Host.
--
-- NOT NULL with no default: every account has always been founded with a handle
-- from this migration onward, and there is no pre-alpha account data to backfill.
-- The unique index is the store-level backstop for the handler's duplicate-handle
-- pre-check (a race that slips past the read hits this constraint).
ALTER TABLE accounts ADD COLUMN handle text NOT NULL;

CREATE UNIQUE INDEX accounts_handle_key ON accounts (handle);
