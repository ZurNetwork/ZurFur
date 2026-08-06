-- The flat commission composition model (ZMVP-166; Flat Composition DD
-- DESIGN/45514754, amended 2026-08-04). This RETIRES the recursive
-- `commission_node` adjacency tree (ZMVP-71/72/73, Tree Storage DD 28409880)
-- and replaces it with three fixed-depth tables:
--
--   commission_tab           one row per (commission, declared tab), carrying
--                            the tab's visibility mode
--   commission_element       the leaves — typed contributions INTO a
--                            core-declared surface, addressed by (tab, surface)
--   commission_surface_mode  the per-commission mode of a code-declared surface
--                            (absent row = Total, the closed door)
--
-- There are **no parent pointers anywhere**. Depth is fixed by the model, not by
-- data: commission (the formal root) > tab > surface > element, so effective
-- visibility is `min(tab.mode, surface_mode, element.mode)` under the
-- commission's own `visibility` column — three lookups, fixed arity, zero
-- recursion. The recursive tree's whole failure class (orphans, cycles,
-- unbounded depth, min-of-ancestors walks) is unrepresentable rather than
-- guarded.
--
-- **Surface STRUCTURE is code, surface MODES are data.** Which surfaces exist —
-- and which tab each lives in — is a global, invariant skeleton declared in Rust
-- (`domain::elements::commission::element::SKELETON`), so there is no row to
-- create or delete for a surface and no commission can have a different set.
-- Only a surface's per-commission *mode* is data, and only once it has been
-- widened past the default (hence: absent row = Total).
--
-- **Drop-and-recreate, not a data migration** (Engineer ruling 2026-08-04:
-- pre-alpha, no real data). Existing dev databases keep their commissions and
-- every envelope fact; they lose, exhaustively:
--
--   * their tree content (every `commission_node` row);
--   * their declared Slots and Seats (carried BY nodes that no longer exist);
--   * every seat invitation, pending or historical;
--   * **every non-owner participant row** — see the membership sweep below.
--
-- What IS backfilled is the skeleton: every existing commission gets its tab
-- rows minted here, the same retroactive half the ZMVP-71 root backfill did, so
-- "a commission always has its tab state" holds for old rows too.
--
-- ⚠️ **Membership dies with the seats that justified it** (Engineer ruling
-- 2026-08-05, security finding F1). `commission_participant` is not dropped
-- here, but every seat that ever *granted* a non-owner their membership is —
-- and a participant row is what gets someone through the closed door
-- (`is_participant`, the uniform-404 gate on every commission route). Left
-- alone, those rows would outlive their justification: a User who was seated on
-- a pre-flat commission would keep full participant read access to a
-- composition they now have no seat in, with no row left anywhere explaining
-- why. So the residue is swept — the owner's permanent floor row survives, and
-- ZMVP-79's acceptance re-earns every other membership from a seat that
-- actually exists.

-- ─── Retire the tree ───────────────────────────────────────────────────────
--
-- Order matters: `commission_invitation.seat_id` references `commission_seat`,
-- which references `commission_node`, as does `commission_slot`.
--
-- The invitation TABLE survives (its shape is unchanged — the offer is still
-- (commission, seat, invited user)); only its rows and its seat foreign key go,
-- because every seat they pointed at is about to be dropped. The key is
-- re-established below against the recreated satellite — transitively
-- repointed, not redefined.
DELETE FROM commission_invitation;
ALTER TABLE commission_invitation
    DROP CONSTRAINT commission_invitation_seat_id_fkey;

DROP TABLE commission_slot;
DROP TABLE commission_seat;
DROP TABLE commission_node;

-- ─── The membership residue ────────────────────────────────────────────────
--
-- Engineer ruling 2026-08-05 (security finding F1): dropping every Seat drops
-- every *justification* for non-owner membership, so the membership goes with
-- it. A `commission_participant` row is the key to the closed door — it is what
-- `is_participant` answers `true` on, and every commission route answers a
-- `false` with the uniform 404. Keeping rows whose seats no longer exist would
-- leave stale participants reading a composition they hold no position in.
--
-- The owner's row is the exception and stays: it never came from a Seat. It is
-- the permanent floor (ZMVP-76) — inserted with the commission, irremovable, and
-- re-asserted by the owner-floor trigger — so `c.owner_id` is exactly the line
-- between "membership a Seat granted" and "membership ownership grants".
--
-- Everyone else re-earns membership through ZMVP-79's acceptance, from a seat
-- that actually exists.
DELETE FROM commission_participant p
USING commission c
WHERE c.id = p.commission_id AND p.user_id <> c.owner_id;

-- ─── commission_tab ────────────────────────────────────────────────────────
--
-- A Tab is a Surface of kind tab (DD: tabs are one level up, not a different
-- species) — the only composition level carrying a row of its own, because its
-- mode is per-commission data and elements must reference it by a real key.
--
-- id             The tab's key. App-minted UUIDv7 for tabs minted with their
--                commission; the backfill below mints v4 (PG16 has no uuidv7();
--                a backfilled skeleton row needs no time-sortability).
-- commission_id  The commission this tab belongs to. ON DELETE CASCADE: tabs are
--                commission-owned composition, NOT facts (Deletion DD 3014657) —
--                ZMVP-66's "gone entirely" relies on the cascade.
-- tab            The declared tab's stable id from the code skeleton ('main'
--                today). text, not a pg enum: the tab vocabulary is the type
--                catalog's (ZMVP-171) to grow without a migration.
-- mode           The tab's visibility mode ('presentation' | 'description' |
--                'total'), NOT NULL and defaulting to the closed door. One of
--                the three terms of the effective-visibility min; NOT nullable,
--                because a tab whose mode is absent would have to mean
--                something, and "err closed" is better said once, in the
--                DEFAULT.
--
-- UNIQUE (commission_id, tab)  one row per declared tab per commission.
-- UNIQUE (id, commission_id)   redundant on its own (id is already unique) and
--                              deliberately so: it is the key
--                              `commission_element`'s COMPOSITE foreign key
--                              targets, which is what makes an element citing
--                              ANOTHER commission's tab unrepresentable at the
--                              database rather than caught by an application
--                              check. Do not drop it.
CREATE TABLE commission_tab (
    id            uuid PRIMARY KEY,
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    tab           text NOT NULL,
    mode          text NOT NULL DEFAULT 'total',
    UNIQUE (commission_id, tab),
    UNIQUE (id, commission_id)
);

-- The whole-commission read: SELECT … WHERE commission_id = $1.
CREATE INDEX commission_tab_by_commission ON commission_tab (commission_id);

-- ─── commission_element ────────────────────────────────────────────────────
--
-- An Element is a typed contribution INTO a declared surface: a core-owned
-- envelope Postgres can constrain and audit, plus a type-owned `payload` the
-- core never interprets (the type catalog interprets it later — ZMVP-171).
-- Elements are leaves, always. Nothing is ever an element's parent but the
-- (tab, surface) pair it names.
--
-- id             The element key, app-minted UUIDv7 (time-sortable).
-- commission_id  The owning commission. ON DELETE CASCADE (non-fact, as above),
--                and the second half of the composite key below.
-- tab_id         The tab this element sits in — by id, never by a parent chain.
-- surface        The code-declared surface it was contributed into. text, and
--                deliberately NOT a foreign key: surfaces have no rows (their
--                structure is code). The skeleton is the authority, checked in
--                the domain and enforced by BOTH adapters (`UnknownSurface`).
-- type           The element's type tag from the deferred catalog (ZMVP-171).
--                Open text in v1 — the core stores and returns it, never
--                interprets it.
-- mode           The element's own visibility mode — the third term of the min.
--                NOT NULL, defaulting to the closed door, exactly like a tab's.
-- band           Ordering band within a surface: the reserved column that lets
--                one surface hold several independently-ordered runs.
--                ⚠️ PLACEHOLDER VOCABULARY: the band vocabulary is UNDECIDED,
--                pending the type-catalog DD (ZMVP-171) — Engineer ruling
--                2026-08-04. Everything is born in 'body' and nothing offers a
--                way to choose otherwise yet; the column is reserved so that
--                decision needs no migration of existing rows.
-- position       Integer order within (tab, surface, band). Append = max + 1,
--                assigned in-transaction; a removal renumbers the group.
-- created_by     The acting User — per-element FK teeth for authorization and
--                audit (the plugin-attribution hook ZMVP-167 will read).
-- created_at     When the element was contributed. Application-supplied (no
--                DEFAULT now()), matching the codebase convention.
-- payload        The type-owned half, schemaless at the DB layer by design.
--
-- FOREIGN KEY (tab_id, commission_id) → commission_tab (id, commission_id):
--   THE SECURITY PROPERTY, and it is structural. A composite key means the tab
--   an element cites must belong to the SAME commission the element does; an
--   element pointing at another commission's tab cannot be written at all — not
--   refused by a check some future code path might skip, but unrepresentable.
--   Keep it composite. ON DELETE CASCADE: a tab takes its elements with it.
--
-- UNIQUE (commission_id, tab_id, surface, band, position) DEFERRABLE:
--   ordering is a total order within the band. DEFERRABLE INITIALLY DEFERRED so
--   a renumbering UPDATE may pass through intermediate collisions inside one
--   transaction (the ZMVP-73 precedent).
--
-- UNIQUE (id, commission_id):
--   redundant on its own (id is already the primary key) and deliberately so —
--   the same move commission_tab makes one level up, for the same reason. It is
--   the key the SATELLITES' composite foreign keys target, which is what makes a
--   Slot or Seat claiming a different commission than its own element
--   unrepresentable at the database instead of trusted to application code. Do
--   not drop it.
CREATE TABLE commission_element (
    id            uuid        PRIMARY KEY,
    commission_id uuid        NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    tab_id        uuid        NOT NULL,
    surface       text        NOT NULL,
    type          text        NOT NULL,
    mode          text        NOT NULL DEFAULT 'total',
    band          text        NOT NULL DEFAULT 'body',
    position      integer     NOT NULL,
    created_by    uuid        NOT NULL REFERENCES users (id),
    created_at    timestamptz NOT NULL,
    payload       jsonb       NOT NULL DEFAULT '{}'::jsonb,
    FOREIGN KEY (tab_id, commission_id)
        REFERENCES commission_tab (id, commission_id) ON DELETE CASCADE,
    UNIQUE (commission_id, tab_id, surface, band, position) DEFERRABLE INITIALLY DEFERRED,
    UNIQUE (id, commission_id)
);

-- The whole-commission read: SELECT … WHERE commission_id = $1.
CREATE INDEX commission_element_by_commission ON commission_element (commission_id);

-- ─── commission_surface_mode ───────────────────────────────────────────────
--
-- A code-declared surface's per-commission visibility mode — the second term of
-- the min. Surfaces have no rows of their own (their structure is code), so the
-- only surface *data* is this override, and an ABSENT ROW MEANS TOTAL: a
-- commission that has never widened a surface is fully closed by having said
-- nothing. Widening is an explicit later act (ZMVP-74 owns the write surface);
-- this migration lays the table down and nothing in ZMVP-166 writes to it.
--
-- commission_id  The commission. ON DELETE CASCADE (non-fact, as above).
-- surface        The code-declared surface id the mode applies to. text, no FK,
--                for the same reason as commission_element.surface.
-- mode           The surface's mode for this commission. NOT NULL: absence is
--                spelled by the row not existing, never by a NULL, so there is
--                exactly one way to say "default".
CREATE TABLE commission_surface_mode (
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    surface       text NOT NULL,
    mode          text NOT NULL,
    PRIMARY KEY (commission_id, surface)
);

-- ─── The satellites, recreated on element identity ─────────────────────────
--
-- Slot and Seat are identity-sharing satellites (Gate A ruling E20): declaring
-- one plants an ordinary element and hangs the interpreted data off that
-- element's id. Only the key they hang from moved — node → element; their
-- columns and cascade semantics are otherwise exactly as ZMVP-77/76 wrote them.
--
-- Their element key is COMPOSITE, and that is the change:
--
--   FOREIGN KEY (<element key>, commission_id)
--       REFERENCES commission_element (id, commission_id) ON DELETE CASCADE
--
-- replacing the plain `REFERENCES commission_element (id)`. Same doctrine as the
-- element→tab edge one level up: a satellite that claims a DIFFERENT commission
-- than the element it hangs off is now **unrepresentable**, not merely never
-- written. It matters because `commission_slot.commission_id` /
-- `commission_seat.commission_id` are what the by-commission reads filter on
-- (`seats()`, `slots_of`) — a desynced pair would list one commission's Seat
-- among another's, past every application check, and be projected under the
-- wrong commission's visibility. With the composite key the database refuses the
-- row (SQLSTATE 23503) even from raw SQL.
--
-- The plain `REFERENCES commission (id)` on `commission_id` STAYS, and is now
-- strictly redundant (the composite key reaches `commission` transitively
-- through the element). Kept deliberately: it states the satellite's own
-- ownership without making a reader chase a second table's constraints to learn
-- it, and it keeps the commission-delete cascade a direct edge rather than one
-- that only works via the element. Same reasoning as commission_tab's redundant
-- UNIQUE (id, commission_id).

-- Declared Slots (ZMVP-77; DESIGN/Slots 5931025, DD 28311564): the required
-- title and optional freeform notes of a Character position. Still deliberately
-- occupant-less — filling a Slot is the Character epic's, and an empty Slot is a
-- valid, PERMANENT state.
CREATE TABLE commission_slot (
    element_id    uuid NOT NULL PRIMARY KEY,
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    title         text NOT NULL CHECK (btrim(title) <> ''),
    notes         text,
    FOREIGN KEY (element_id, commission_id)
        REFERENCES commission_element (id, commission_id) ON DELETE CASCADE
);

CREATE INDEX commission_slot_by_commission ON commission_slot (commission_id);

-- The Seat's interpreted half (ZMVP-76; DD 28311564): kind, requirements, and
-- THE occupancy model — one nullable column, so "a Seat holds at most one
-- occupant" stays unrepresentable to violate. Every seat is born vacant.
CREATE TABLE commission_seat (
    id            uuid PRIMARY KEY,
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    kind          text NOT NULL,
    prompt        text,
    link          text,
    occupant      uuid REFERENCES users (id),
    FOREIGN KEY (id, commission_id)
        REFERENCES commission_element (id, commission_id) ON DELETE CASCADE
);

CREATE INDEX commission_seat_by_commission ON commission_seat (commission_id);

-- The invitation's seat key, re-established against the recreated satellite:
-- pruning a seat's element still sweeps its pending offers with it (ZMVP-78).
ALTER TABLE commission_invitation
    ADD CONSTRAINT commission_invitation_seat_id_fkey
    FOREIGN KEY (seat_id) REFERENCES commission_seat (id) ON DELETE CASCADE;

-- ─── Skeleton backfill ─────────────────────────────────────────────────────
--
-- gen_random_uuid() is core from PG13 but lives in pgcrypto on older servers;
-- harmless where it's already core (the ZMVP-71 migration's note).
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Every commission that predates this migration gets its skeleton tab rows —
-- the retroactive half of "tab state exists explicitly from birth" (new
-- commissions mint theirs in the same unit of work as the commission row,
-- CommissionWrites::create). Mode is left to the column DEFAULT: the closed
-- door, NOT a mapping from `commission.visibility`. The commission's own
-- visibility field stays the outermost gate under which the min applies; a tab
-- born wide would widen composition nobody ever chose to widen.
--
-- ⚠️ The tab list below MIRRORS the code skeleton at the time of writing (one
-- placeholder tab, 'main'). It is a snapshot, not a link: when ZMVP-171 replaces
-- the placeholder skeleton with the real catalog, that change owns minting
-- whatever tabs it adds for existing commissions.
INSERT INTO commission_tab (id, commission_id, tab)
SELECT gen_random_uuid(), c.id, 'main'
FROM commission c;
