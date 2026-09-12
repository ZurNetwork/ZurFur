-- Commission Markup: coordinate-anchored annotation a Participant draws over
-- a file entry in the review loop, given its own canonical row alongside the
-- `markup_added` changelog entry that remains the timeline fact. See NODE.md
-- ("20260902225852_create_commission_markup.sql") for the full rationale.
-- Immutability is a write-port policy, not a database constraint: no update
-- or delete method is exposed on CommissionWrites for this table.

-- Give commission_file a key the markup rows can point at as a PAIR. `id` is
-- already the primary key, so this constraint costs nothing and adds nothing new
-- to enforce — it exists so the composite foreign key below can be declared
-- (the same technique as actor_projection_composite_fk).
ALTER TABLE commission_file
    ADD CONSTRAINT commission_file_id_commission UNIQUE (id, commission_id);

-- id            The markup's own opaque key. App-minted UUIDv7 (PG16 has no
--               native uuidv7()), so it sorts as creation order — the identity a
--               changelog payload never had, and what a future reply or
--               resolution would address.
-- commission_id The stream the markup belongs to. Carried even though file_id
--               implies it, because every read scopes by it: a key from another
--               commission must be invisible rather than a not-authorized signal
--               (the same non-oracle rule commission_file's reads follow).
-- file_id       The annotated file entry. The (file_id, commission_id) composite
--               reference is deliberate — it makes "a markup on a file from a
--               DIFFERENT commission" unrepresentable rather than merely
--               unreachable.
-- added_by      The annotating Participant, as their DID — the actor key
--               everywhere. Deliberately NO foreign key onto the actor
--               tables: shared history survives a tombstone, exactly like
--               commission_file.uploaded_by and the changelog's actor_id.
-- shape         The annotation geometry, validated by domain::…::MarkupShape
--               before it ever reaches here. jsonb because the shape vocabulary
--               is a closed enum today but an open question tomorrow (the future
--               canvas Plugin), and because normalized 0-1 floats are never
--               queried numerically by the server — it stores and serves them.
-- text          The optional comment anchored at the shape. Distinct from the
--               changelog entry's free-text `note`; capped and non-blank by
--               Markup::validate on the way in, stored untransformed.
-- created_at    When the markup was drawn. Application-supplied (no DEFAULT
--               now()), matching the codebase convention.
CREATE TABLE commission_markup (
    id            uuid        PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    file_id       uuid        NOT NULL,
    added_by      text        NOT NULL,
    shape         jsonb       NOT NULL,
    text          text,
    created_at    timestamptz NOT NULL,

    -- Ties the markup to a file entry AND pins both to the same commission in one
    -- constraint. CASCADE: a markup cannot outlive the file it annotates — it
    -- would point at nothing, and its coordinates mean nothing without the image.
    CONSTRAINT commission_markup_file
        FOREIGN KEY (file_id, commission_id)
        REFERENCES commission_file (id, commission_id)
        ON DELETE CASCADE
);

-- The one read this table exists for: every markup on one file entry, in the
-- order it was drawn (UUIDv7 `id` sorts as creation order, so no separate sort
-- key is needed). Scoped by commission_id because every read is.
CREATE INDEX commission_markup_file_entry
    ON commission_markup (commission_id, file_id, id);
