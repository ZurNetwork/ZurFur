-- Actor Addressing — the DID becomes the actor key (DD 57081857).
--
-- Until now an actor had two names: a surrogate UUIDv7 row key and the
-- `did:plc` everything public already used. Decision 1 of the DD collapses them
-- — "did is the primary key of every actor table, and every FK to an actor is a
-- DID column" — so `users.id` and `accounts.id` become the DID text, and every
-- column that pointed at an actor by UUID is retyped to carry the DID instead.
-- Bluesky's own PDS/AppView/Ozone key their actor tables `did … PRIMARY KEY`;
-- this migration adopts the same shape.
--
-- DATA-PRESERVING. Nothing is dropped and nothing is invented: `actor_identity`
-- already holds the (id → did) map for every actor (migration
-- 20260718193956 backfilled it, and 20260718194001's CHECK makes a user/account
-- identity's DID NOT NULL), so every UUID in the schema can be rewritten to the
-- DID it already stood for. `actor_did()` below does that lookup once and
-- RAISEs — loudly, naming the offending UUID — rather than inventing a value if
-- an actor ever fails to resolve.
--
-- WHAT IS *NOT* DONE HERE. `actor_identity` keeps its own `id uuid PRIMARY KEY`.
-- The DD would have that surrogate go too, but `ActorIdentityId` is still a UUID
-- newtype in the domain and `ActorIdentity.did` is still `Option<Did>` (the
-- super-table admits DID-less kinds), so re-keying it is a domain change, not an
-- adapter one. The projections' parent constraint is repointed at the natural
-- key — `users (id, kind) → actor_identity (did, kind)` — so the ZMVP-123
-- invariant ("a projection row has an identity parent of matching kind") is kept
-- exactly, addressed by DID like everything else.

-- The one lookup this migration is built on: the DID an actor UUID already
-- stood for. NULL in, NULL out (nullable actor columns stay nullable); a UUID
-- with no identity row — or an identity carrying no DID — aborts the migration
-- naming the row, because silently coining an identifier is the one outcome
-- worse than failing to migrate.
CREATE FUNCTION actor_did(actor uuid) RETURNS text
LANGUAGE plpgsql STABLE AS $$
DECLARE
    resolved text;
BEGIN
    IF actor IS NULL THEN
        RETURN NULL;
    END IF;

    SELECT ai.did INTO resolved FROM actor_identity ai WHERE ai.id = actor;

    IF resolved IS NULL THEN
        RAISE EXCEPTION
            'actor re-key aborted (DD 57081857): actor % has no actor_identity row, or its row carries no DID. '
            'Every actor must resolve to a DID before it can become the key. Reconcile the identity, then re-run.',
            actor;
    END IF;

    RETURN resolved;
END $$;

-- Every foreign key that spans an actor column has to go before the type
-- changes: PostgreSQL will not hold a reference whose two sides disagree on
-- type. They are all re-declared, unchanged in meaning, at the bottom.
ALTER TABLE users                        DROP CONSTRAINT users_actor_identity_fk;
ALTER TABLE accounts                     DROP CONSTRAINT accounts_actor_identity_fk;
ALTER TABLE account_members              DROP CONSTRAINT account_members_account_id_fkey;
ALTER TABLE account_members              DROP CONSTRAINT account_members_user_id_fkey;
ALTER TABLE account_members              DROP CONSTRAINT account_members_parent_fkey;
ALTER TABLE account_invitations          DROP CONSTRAINT account_invitations_account_id_fkey;
ALTER TABLE account_invitations          DROP CONSTRAINT account_invitations_invited_user_fkey;
ALTER TABLE account_invitations          DROP CONSTRAINT account_invitations_inviter_fkey;
ALTER TABLE account_handle_changes       DROP CONSTRAINT account_handle_changes_account_id_fkey;
ALTER TABLE commission                   DROP CONSTRAINT commission_owner_id_fkey;
ALTER TABLE commission_participant       DROP CONSTRAINT commission_participant_user_id_fkey;
ALTER TABLE commission_placement         DROP CONSTRAINT commission_placement_account_id_fkey;
ALTER TABLE commission_current_placement DROP CONSTRAINT commission_current_placement_account_id_fkey;
ALTER TABLE commission_view_grant        DROP CONSTRAINT commission_view_grant_account_id_fkey;
ALTER TABLE commission_element           DROP CONSTRAINT commission_element_created_by_fkey;
ALTER TABLE commission_seat              DROP CONSTRAINT commission_seat_occupant_fkey;
ALTER TABLE commission_invitation        DROP CONSTRAINT commission_invitation_invited_user_fkey;
ALTER TABLE commission_invitation        DROP CONSTRAINT commission_invitation_inviter_fkey;

-- The two actor tables. Their primary keys, their indexes (accounts_handle_key,
-- account_members_by_user, commission_by_owner, the pending-invitation partial
-- uniques) and their NOT NULL markers all survive the retype — PostgreSQL
-- rebuilds an index in place when a column it covers changes type.
ALTER TABLE users    ALTER COLUMN id TYPE text USING actor_did(id);
ALTER TABLE accounts ALTER COLUMN id TYPE text USING actor_did(id);

-- Everything that points at an actor. Grouped by table, in schema order.
ALTER TABLE account_members
    ALTER COLUMN account_id TYPE text USING actor_did(account_id),
    ALTER COLUMN user_id    TYPE text USING actor_did(user_id),
    -- Nullable — the role tree's root has no parent (DESIGN/Roles rule 5).
    ALTER COLUMN parent     TYPE text USING actor_did(parent);

ALTER TABLE account_invitations
    ALTER COLUMN account_id   TYPE text USING actor_did(account_id),
    ALTER COLUMN invited_user TYPE text USING actor_did(invited_user),
    ALTER COLUMN inviter      TYPE text USING actor_did(inviter);

ALTER TABLE account_handle_changes
    ALTER COLUMN account_id TYPE text USING actor_did(account_id);

ALTER TABLE commission
    ALTER COLUMN owner_id TYPE text USING actor_did(owner_id);

-- Nullable and deliberately un-keyed: a system-appended entry has no actor, and
-- shared history outlives a tombstoned one (the same reasoning that keeps
-- commission_file.uploaded_by free of a foreign key).
ALTER TABLE commission_changelog
    ALTER COLUMN actor_id TYPE text USING actor_did(actor_id);

ALTER TABLE commission_file
    ALTER COLUMN uploaded_by TYPE text USING actor_did(uploaded_by);

ALTER TABLE commission_participant
    ALTER COLUMN user_id TYPE text USING actor_did(user_id);

ALTER TABLE commission_placement
    ALTER COLUMN account_id TYPE text USING actor_did(account_id),
    ALTER COLUMN placed_by  TYPE text USING actor_did(placed_by);

ALTER TABLE commission_current_placement
    ALTER COLUMN account_id TYPE text USING actor_did(account_id),
    ALTER COLUMN placed_by  TYPE text USING actor_did(placed_by);

-- `account_id` becomes `grantee`, and loses its foreign key.
--
-- Which actor CLASS holds a view grant is in flux: the write port now issues one
-- to a User (`grant_view(commission, to_user, level)`, per the Engineer's
-- 2026-09-04 ruling amended inline onto Ownership Separation DD 29130754 —
-- "view grants are issued to Users; membership confers no view"), while the read
-- port still asks by `AccountId` and `AccountWrites::hard_delete` still
-- describes grants as account rails severed by cascade. Rows written under the
-- old reading exist and point at accounts; rows written under the new one point
-- at users, so no single reference can hold both — and re-pointing the column at
-- `users` would mean DELETING the account-keyed rows, which is a decision for
-- the Engineer, not a side effect of a re-key.
--
-- So the column stores an actor's DID and asserts nothing about its class. That
-- is not a new posture: `commission_changelog.actor_id`,
-- `commission_file.uploaded_by` and `commission_placement.placed_by` already
-- carry an actor without a foreign key. The name follows suit — `grantee` is the
-- role the DID plays, true under either reading, where `account_id` is now a
-- claim the data may not support.
ALTER TABLE commission_view_grant
    ALTER COLUMN account_id TYPE text USING actor_did(account_id);
ALTER TABLE commission_view_grant
    RENAME COLUMN account_id TO grantee;

ALTER TABLE commission_element
    ALTER COLUMN created_by TYPE text USING actor_did(created_by);

-- Nullable — a declared Seat is born vacant (ZMVP-76).
ALTER TABLE commission_seat
    ALTER COLUMN occupant TYPE text USING actor_did(occupant);

ALTER TABLE commission_invitation
    ALTER COLUMN invited_user TYPE text USING actor_did(invited_user),
    ALTER COLUMN inviter      TYPE text USING actor_did(inviter);

-- The projections' parent constraint, repointed from the surrogate onto the
-- natural key. `did` is nullable on actor_identity (DID-less kinds are still
-- representable there), which a UNIQUE constraint permits and a foreign key is
-- happy to reference; the CHECK added in 20260718194001 already forces a
-- user/account identity to carry one, so a projection row can never point at a
-- DID-less parent.
ALTER TABLE actor_identity
    ADD CONSTRAINT actor_identity_did_kind_key UNIQUE (did, kind);

ALTER TABLE users
    ADD CONSTRAINT users_actor_identity_fk
        FOREIGN KEY (id, kind) REFERENCES actor_identity (did, kind);
ALTER TABLE accounts
    ADD CONSTRAINT accounts_actor_identity_fk
        FOREIGN KEY (id, kind) REFERENCES actor_identity (did, kind);

ALTER TABLE account_members
    ADD CONSTRAINT account_members_account_id_fkey
        FOREIGN KEY (account_id) REFERENCES accounts (id),
    ADD CONSTRAINT account_members_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES users (id),
    ADD CONSTRAINT account_members_parent_fkey
        FOREIGN KEY (parent) REFERENCES users (id);

ALTER TABLE account_invitations
    ADD CONSTRAINT account_invitations_account_id_fkey
        FOREIGN KEY (account_id) REFERENCES accounts (id),
    ADD CONSTRAINT account_invitations_invited_user_fkey
        FOREIGN KEY (invited_user) REFERENCES users (id),
    ADD CONSTRAINT account_invitations_inviter_fkey
        FOREIGN KEY (inviter) REFERENCES users (id);

ALTER TABLE account_handle_changes
    ADD CONSTRAINT account_handle_changes_account_id_fkey
        FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE;

ALTER TABLE commission
    ADD CONSTRAINT commission_owner_id_fkey
        FOREIGN KEY (owner_id) REFERENCES users (id);

ALTER TABLE commission_participant
    ADD CONSTRAINT commission_participant_user_id_fkey
        FOREIGN KEY (user_id) REFERENCES users (id);

ALTER TABLE commission_placement
    ADD CONSTRAINT commission_placement_account_id_fkey
        FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE;

ALTER TABLE commission_current_placement
    ADD CONSTRAINT commission_current_placement_account_id_fkey
        FOREIGN KEY (account_id) REFERENCES accounts (id) ON DELETE CASCADE;

ALTER TABLE commission_element
    ADD CONSTRAINT commission_element_created_by_fkey
        FOREIGN KEY (created_by) REFERENCES users (id);

ALTER TABLE commission_seat
    ADD CONSTRAINT commission_seat_occupant_fkey
        FOREIGN KEY (occupant) REFERENCES users (id);

ALTER TABLE commission_invitation
    ADD CONSTRAINT commission_invitation_invited_user_fkey
        FOREIGN KEY (invited_user) REFERENCES users (id),
    ADD CONSTRAINT commission_invitation_inviter_fkey
        FOREIGN KEY (inviter) REFERENCES users (id);

DROP FUNCTION actor_did(uuid);
