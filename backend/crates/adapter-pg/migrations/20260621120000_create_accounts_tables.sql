-- An Account and its membership roster (see ZMVP-14, DESIGN/Account, DESIGN/Roles).
-- An account is a sovereign entity: it carries both an app-internal UUIDv7 key and
-- its own did:plc. It is founded by a User who, in the same act, becomes its Owner.
-- Both rows below are written in a single private-side transaction (no cross-store
-- dual write — the did:plc minting that precedes them is the separate, retryable step).
--
-- id          UUIDv7 minted app-side; opaque internal key, never crosses the public
--             boundary. (Same convention as users.id.)
-- did         The account's did:plc. UNIQUE: one DID, one account. FLOOR STUB —
--             currently a synthetic value from the stub DidMinter; the real minted
--             did:plc lands when that port is dressed.
-- name        The founder-supplied name, e.g. 'Acme Studio'. Validated in the domain
--             (AccountName): trimmed, non-empty, <= 120 chars. The CHECK is a second
--             line of defense. This is the account's *private* record of its name;
--             publishing it as a public profile record to the account's PDS is a
--             later step (the did:plc itself is still the floor stub).
-- created_at  When the account was founded. Application-supplied, no DEFAULT now().
-- updated_at  Last mutation to account-level facts. Founded equal to created_at.
-- deleted_at  Soft-delete tombstone (DESIGN/Account: "never hard-deleted on our
--             side"). NULL = live. find() treats a non-NULL value as absent.
CREATE TABLE accounts (
    id         uuid        PRIMARY KEY,
    did        text        NOT NULL UNIQUE,
    name       text        NOT NULL CHECK (char_length(btrim(name)) BETWEEN 1 AND 120),
    created_at timestamptz NOT NULL,
    updated_at timestamptz NOT NULL,
    deleted_at timestamptz
);

-- Who belongs to an account and in what role. A user may belong to many accounts
-- and an account to many users, so membership is its own table keyed by the pair.
--
-- role    The role discriminant: 'owner' | 'admin' | 'manager' | 'member'
--         (DESIGN/Roles). Stored as text — a small, human-legible closed set.
-- parent  The member's parent in the account's role tree, mirroring the domain
--         Role's Option<String> slot (DESIGN/Roles: parent-ship by invitation or
--         promotion). NULL for an Owner — "an Owner never has a parent." The
--         hierarchy mechanics (and any FK/semantics for this column) arrive with
--         their own ticket; the floor only ever writes an Owner, so it is NULL.
CREATE TABLE account_members (
    account_id uuid NOT NULL REFERENCES accounts (id),
    user_id    uuid NOT NULL REFERENCES users (id),
    role       text NOT NULL,
    parent     text,
    PRIMARY KEY (account_id, user_id)
);
