-- actor_identity: the kind column —
-- the closed actor vocabulary, and the UNIQUE (id, kind) anchor every
-- kind-checked reference site's composite FK will target.
-- NOT NULL: fills happen in app contexts where the kind is known; the
-- unknown-kind representation for bare network DIDs is left for a later change.
ALTER TABLE actor_identity
    ADD COLUMN kind text NOT NULL
        CHECK (kind IN ('user', 'account', 'character'));

ALTER TABLE actor_identity
    ADD CONSTRAINT actor_identity_id_kind_key UNIQUE (id, kind);
