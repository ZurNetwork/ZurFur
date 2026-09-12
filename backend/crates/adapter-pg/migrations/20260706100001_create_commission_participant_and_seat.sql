-- Participant membership + the Seat satellite. Participant-hood is PERSISTED,
-- never computed as owner ∪ seat-occupants — "the prior owner remains
-- a Participant" after an ownership transfer would be unrepresentable in a
-- computed model.

-- ─── commission_participant ────────────────────────────────────────────────
--
-- One row per (commission, User) membership: the record CommissionStore::
-- is_participant reads. The owner's row is inserted with the commission itself
-- (CommissionWrites::create) and backfilled below for commissions that predate
-- the table. Accepted seat invitations add
-- seated members as further rows; ownership transfer moves only
-- commission.owner_id and leaves the old owner's row in place.
--
-- commission_id  The commission the membership belongs to. ON DELETE CASCADE:
--                membership is commission-owned bookkeeping, NOT a fact —
--                the commission's "gone entirely" hard-delete relies on
--                the cascade sweeping it.
-- user_id        The member. Participants are always Users, never accounts.
--                FK onto users(id): membership is living
--                state, not history (unlike changelog actor_id).
-- created_at     When the membership began. Application-supplied (no DEFAULT
--                now()), matching the codebase convention; the owner's row
--                carries the commission's own creation instant.
--
-- The natural composite key IS the identity (a membership is a pair, like
-- account_members) — serial-by-design, so no UUIDv7 surrogate.
CREATE TABLE commission_participant (
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    user_id       uuid        NOT NULL REFERENCES users (id),
    created_at    timestamptz NOT NULL,
    PRIMARY KEY (commission_id, user_id)
);

-- Owner backfill: every commission created before the table existed gets its owner's
-- membership row here, stamped with the commission's own creation instant. New
-- commissions never reach this path: their owner row is inserted in the same
-- unit of work as the commission row (CommissionWrites::create).
INSERT INTO commission_participant (commission_id, user_id, created_at)
SELECT c.id, c.owner_id, c.created_at
FROM commission c;

-- The permanent floor: a commission always has at least one Participant, its
-- owner, and the owner's membership row is IRREMOVABLE while its
-- commission lives. No port or route removes a participant at all today; this
-- trigger makes removing the owner's row unreachable even for future code
-- reaching past the ports (the commission_changelog append-only precedent).
-- The commission hard-delete cascade still sweeps the row: cascaded
-- child deletes run after the commission row itself is gone, so the EXISTS
-- probe below finds nothing and lets them through. Non-owner rows (accepted
-- seat invitations' seated members) never match the probe and stay freely removable.
CREATE FUNCTION commission_participant_refuse_owner_delete() RETURNS trigger AS $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM commission
        WHERE id = OLD.commission_id AND owner_id = OLD.user_id
    ) THEN
        RAISE EXCEPTION
            'the owner is a permanent Participant: their membership row leaves only with the commission itself (ZMVP-76)';
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER commission_participant_owner_floor
    BEFORE DELETE ON commission_participant
    FOR EACH ROW EXECUTE FUNCTION commission_participant_refuse_owner_delete();

-- ─── commission_seat ───────────────────────────────────────────────────────
--
-- The Seat's interpreted half: in the tree a Seat is an
-- ordinary component node (untyped v1 contract — position + visibility
-- inheritance, empty payload); the typed data the core MUST interpret — kind,
-- requirements, occupancy — lives here, keyed by that node's id, with real
-- columns Postgres can constrain and later tickets can FK against.
--
-- id             The seat node's id — one identity, two rows. ON DELETE
--                CASCADE from the node: removing the seat's subtree
--                sweeps the satellite with it.
-- commission_id  The owning commission, denormalized for the one-query seats()
--                read and for a DIRECT cascade onto commission(id): the
--                commission's hard-delete must sweep seats even though it
--                predates this table.
-- kind           The seat's semantic kind — an OPEN vocabulary (NOT the Role
--                enum; kinds repeat freely), validated app-side
--                (SeatKind: trimmed, non-empty, capped, no control chars).
--                text, not a pg enum, by construction.
-- prompt         Optional free-text requirements riding the vacant seat
--                (validated app-side, SeatPrompt).
-- link           Optional external requirements link (e.g. a form; responses
--                live off-platform; validated app-side, SeatLink).
-- occupant       THE occupancy model: a single nullable column, so "a Seat
--                holds at most one occupant" is unrepresentable to
--                violate. NULL = vacant (every seat at declaration); an
--                accepted invitation fills it. FK onto users(id): occupancy is
--                living state.
CREATE TABLE commission_seat (
    id            uuid PRIMARY KEY REFERENCES commission_node (id) ON DELETE CASCADE,
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    kind          text NOT NULL,
    prompt        text,
    link          text,
    occupant      uuid REFERENCES users (id)
);

-- The one read: a commission's seats (CommissionStore::seats).
CREATE INDEX commission_seat_by_commission ON commission_seat (commission_id);
