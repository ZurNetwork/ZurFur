-- The direction-axis Status (ZMVP-85; DESIGN/Commission, Status): whose turn
-- the work is waiting on — waiting_for_input / waiting_for_approval /
-- changes_requested, validated by the domain enum (DirectionStatus — the closed
-- vocabulary; text, not a pg enum, matching lifecycle/visibility). One nullable
-- column (Engineer ruling E29): the axis holds at most one value, so a set
-- REPLACES by construction and NULL = cleared. Moved only by an explicit
-- Participant act (Engineer ruling 2026-07-01 — never a content event or system
-- sweep); each change lands with a status_changed changelog entry in the same
-- transaction. The deadline axis (Delayed/Late, system-set) is ZMVP-86's,
-- deliberately not this column.
ALTER TABLE commission
    ADD COLUMN direction_status text;
