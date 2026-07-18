-- ZMVP-123 slice 2 (DD 34013187 decision 4): bind each projection table to the
-- actor super-table with a COMPOSITE, kind-checked foreign key, so a projection row
-- of the wrong actor kind — or with no identity row at all — is UNREPRESENTABLE, not
-- merely checked. Runs after the backfill, so every existing row already has its
-- (id, kind) parent.
--
-- The `kind` discriminant column exists solely to carry the composite FK; it is
-- pinned to the table's one legal value by a CHECK and filled by a constant column
-- DEFAULT. That default is deliberate and permanent (not the ZMVP-122 backfill-then-
-- drop pattern): it is the mechanism that keeps `kind` invisible to the API/queries
-- — no INSERT ever names it, none can set it wrong (the CHECK rejects any other
-- value), and the composite FK reads it. `(id, kind)` targets the super-table's
-- `actor_identity_id_kind_key` UNIQUE (id, kind) anchor.

-- users → actor_identity (kind = 'user').
ALTER TABLE users
    ADD COLUMN kind text NOT NULL DEFAULT 'user' CHECK (kind = 'user');
ALTER TABLE users
    ADD CONSTRAINT users_actor_identity_fk
        FOREIGN KEY (id, kind) REFERENCES actor_identity (id, kind);

-- accounts → actor_identity (kind = 'account').
ALTER TABLE accounts
    ADD COLUMN kind text NOT NULL DEFAULT 'account' CHECK (kind = 'account');
ALTER TABLE accounts
    ADD CONSTRAINT accounts_actor_identity_fk
        FOREIGN KEY (id, kind) REFERENCES actor_identity (id, kind);

-- Per-kind DID guarantee (DD 34013187 follow-up — "once ZMVP-123 lands and the
-- kinds' DID guarantees are settled"). Users and accounts ALWAYS carry a DID: their
-- former `did NOT NULL` columns are about to be dropped, and this CHECK is where that
-- invariant now lives — one User/Account is one present, UNIQUE DID, forever. Only
-- Characters (and any future DID-less kind) may leave `did` NULL. Every backfilled
-- user/account row satisfies it (their DIDs were NOT NULL), so it validates clean.
ALTER TABLE actor_identity
    ADD CONSTRAINT actor_identity_kind_requires_did
        CHECK (kind NOT IN ('user', 'account') OR did IS NOT NULL);
