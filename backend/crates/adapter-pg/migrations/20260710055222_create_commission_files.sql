-- Commission file entries (ZMVP-88; DESIGN/Commission — "File entries and Markup"):
-- intermediate work-in-progress a Participant uploads into the review loop. A file
-- entry is NOT a Product — no fact-lock, no atproto: it is private, Index-side,
-- Total-tier content. Two tables, kept deliberately apart:
--
--   commission_file  The Index-canonical LINK: which commission a file entry
--                    belongs to, and who uploaded it. The retrieval path reads this
--                    to authorize a participant (scoped to commission_id, so a key
--                    from another commission is invisible here — never an existence
--                    oracle) and it is what the hard-delete cascade (ZMVP-66) reads
--                    to enumerate a commission's blobs. Classified NON-FACT in
--                    adapter-pg/src/commission.rs (COMMISSION_NON_FACT_TABLES): it is
--                    commission-owned bookkeeping that cascades away, so a commission
--                    with only file entries stays hard-deletable (AC2).
--
--   file_blob        The FileStore's v1 local implementation (AC4): the bytes and
--                    caller metadata, keyed by the SAME opaque UUIDv7 handle. It holds
--                    NO foreign key onto commission on purpose — blobs know nothing of
--                    commissions (the link is commission_file). This is the mock/local
--                    store; the real blob architecture (storage, limits, formats,
--                    retention, content-addressing) is the future blob walkthrough,
--                    and swaps behind the FileStore port with these keys still valid.

-- id           The file entry's opaque key — also the file_blob key and the
--              FileStore handle. App-minted UUIDv7 (PG16 has no native uuidv7()).
-- commission_id The stream the entry belongs to. ON DELETE CASCADE: a file entry is
--              bookkeeping, not a fact — it dies with the commission (Deletion DD
--              3014657; Changelog DD retention).
-- uploaded_by  The acting Participant. Deliberately NO foreign key onto users(id):
--              shared history survives a user tombstone, exactly like the changelog's
--              actor_id, so a user row's removal must never be blocked by, nor cascade
--              into, this record.
-- created_at   When the entry was uploaded. Application-supplied (no DEFAULT now()),
--              matching the codebase convention.
CREATE TABLE commission_file (
    id            uuid        PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    uploaded_by   uuid        NOT NULL,
    created_at    timestamptz NOT NULL
);

-- The one read on this table: a commission's entries / a keyed lookup within it.
CREATE INDEX commission_file_commission ON commission_file (commission_id);

-- key          The opaque handle — the same UUIDv7 as commission_file.id.
-- filename     The validated save-name (served in Content-Disposition on download).
-- content_type The normalized MIME (served as Content-Type; never blank/control).
-- byte_size    The stored length.
-- bytes        The content itself. bytea for the v1 local store; a future object
--              store replaces this table behind the FileStore port.
-- created_at   When the blob was stored.
CREATE TABLE file_blob (
    key          uuid        PRIMARY KEY,
    filename     text        NOT NULL,
    content_type text        NOT NULL,
    byte_size    bigint      NOT NULL,
    bytes        bytea       NOT NULL,
    created_at   timestamptz NOT NULL
);
