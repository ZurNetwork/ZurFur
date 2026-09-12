-- The direction-axis Status: whose turn
-- the work is waiting on — waiting_for_input / waiting_for_approval /
-- changes_requested, validated by the domain enum (DirectionStatus — the closed
-- vocabulary; text, not a pg enum, matching lifecycle/visibility). One nullable
-- column: the axis holds at most one value, so a set
-- REPLACES by construction and NULL = cleared. Moved only by an explicit
-- Participant act — never a content event or system
-- sweep; each change lands with a status_changed changelog entry in the same
-- transaction. The deadline axis (Delayed/Late, system-set) is a separate
-- column, deliberately not this one.
ALTER TABLE commission
    ADD COLUMN direction_status text;
