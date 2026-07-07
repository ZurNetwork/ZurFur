-- The commission changelog (ZMVP-87; Changelog DD DESIGN/30408741): the
-- commission's memory — an append-only, immutable, per-commission record of every
-- domain event, and the platform's structured communication channel. Postgres-as-
-- log, following the plc_operations precedent (Kafka-shaped: ordered, immutable,
-- consumer cursors later — cursors/fan-out are ZMVP-100's tables, not columns here).
--
-- seq            Monotonic insertion order — THE ordering key: a commission's
--                stream reads in ascending seq. Surrogate bigserial (not gapless,
--                not a per-commission counter — a gapless counter would serialize
--                writers per commission; the upgrade path is recorded on the port).
-- commission_id  The stream the entry belongs to. ON DELETE CASCADE: entries are
--                commission-owned bookkeeping, NOT facts (Deletion DD 3014657) —
--                they hard-delete only with the commission itself or legal duty
--                (DD retention), which is also why DELETE stays ungoverned below
--                while UPDATE is refused.
-- kind           The entry-taxonomy token, validated by the domain enum
--                (ChangelogEntryKind — the closed vocabulary; text, not a pg enum,
--                so adding a variant is not a migration).
-- actor_id       The acting User, or NULL for a system entry (Delayed/Late).
--                Deliberately NO foreign key onto users(id): entries survive user
--                tombstones (DD retention — shared history is not one party's to
--                erase), so a user row's future removal must never be blocked by,
--                nor cascade into, the record.
-- payload        Kind-specific parameters (jsonb), self-sufficient to render a
--                sentence without joins (the DD's core-renderable rule).
-- note           Optional free text riding the entry — a standalone note entry or
--                one attached to an event (DD Decision 1). Never dialogue.
-- created_at     When the act happened. Application-supplied (no DEFAULT now()),
--                matching the codebase convention; carried for display — seq is
--                the order.
CREATE TABLE commission_changelog (
    seq           bigserial   PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    kind          text        NOT NULL,
    actor_id      uuid,
    payload       jsonb       NOT NULL,
    note          text,
    created_at    timestamptz NOT NULL
);

-- The one read: a commission's stream in order.
CREATE INDEX commission_changelog_commission_seq ON commission_changelog (commission_id, seq);

-- Append-only at the database (ZMVP-87 AC4): entries are never edited. No port or
-- route exposes an update, and this trigger makes one unreachable even for future
-- code reaching past the ports. DELETE is deliberately ungoverned: the commission
-- hard-delete cascade (ZMVP-66) and legal-duty redaction (DD retention) must work.
CREATE FUNCTION commission_changelog_refuse_update() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'commission_changelog is append-only: entries are never edited (ZMVP-87)';
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER commission_changelog_append_only
    BEFORE UPDATE ON commission_changelog
    FOR EACH ROW EXECUTE FUNCTION commission_changelog_refuse_update();

-- The commission's external linked channel — "where we talk" (DD Decision 2): raw
-- pointer text (URL or handle), validated app-side (length cap, no control
-- characters, deliberately no scheme allowlist — it renders as a pointer and never
-- auto-embeds). NULL = no channel declared. Set/clear is owner-only in v1 and
-- changelog-recorded (channel_linked / channel_unlinked).
ALTER TABLE commission
    ADD COLUMN linked_channel text;
