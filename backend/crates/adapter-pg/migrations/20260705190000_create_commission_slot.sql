-- Declared Slots (ZMVP-77; DESIGN/Slots 5931025, Referenceable/Slot/Seat DD
-- 28311564): the SATELLITE half of a Slot. The tree half is an ordinary
-- commission_node component leaf; this table carries the slot's substance —
-- the required title and optional freeform notes — keyed by that node's id
-- (the slot mirror of the Seat satellite ruling, Gate A E20). The generic
-- component add cannot populate this, which is why declaration has its own
-- port/endpoint.
--
-- Deliberately NO occupant column of any kind: filling a Slot is the Character
-- epic's, and an empty Slot is a valid, PERMANENT state (nothing here expires
-- or auto-fills). Adding the occupant is that epic's migration, not a NULL
-- column waiting here.
--
-- node_id        The slot's commission_node row — satellite key = node key.
--                ON DELETE CASCADE: the satellite is meaningless without its
--                node (subtree pruning, ZMVP-73, sweeps it for free).
-- commission_id  The owning commission, denormalized from the node for direct
--                "slots of this commission" reads and for its own cascade:
--                slots are commission-owned bookkeeping, NOT facts (Deletion
--                DD 3014657) — they go when the commission hard-deletes
--                (ruling E35; ZMVP-66 relies on this). Classified in
--                COMMISSION_NON_FACT_TABLES (adapter-pg/src/commission.rs).
-- title          The Slot's required title, validated non-blank app-side
--                (SlotTitle); the CHECK gives the required-ness DB teeth.
-- notes          Optional freeform notes; NULL = none declared (the boundary
--                normalizes blank to absent, so '' never lands).
CREATE TABLE commission_slot (
    node_id       uuid NOT NULL PRIMARY KEY
                       REFERENCES commission_node (id) ON DELETE CASCADE,
    commission_id uuid NOT NULL REFERENCES commission (id) ON DELETE CASCADE,
    title         text NOT NULL CHECK (title <> ''),
    notes         text
);

-- The "slots of this commission" read (and the zero-or-more count).
CREATE INDEX commission_slot_by_commission ON commission_slot (commission_id);
