-- A Slot's name lives on its node alone: the Slot's own title column goes and
-- node_id becomes NOT NULL. First a check proves every Slot has its own node
-- and holds the same title as its node's name; any difference stops the
-- migration, naming the Slot's element id alone. No row is written and the
-- table is not rewritten: PostgreSQL's DROP COLUMN only hides a column. The old
-- titles stay in the commission_slot table's files, where no query or pg_dump
-- returns them but a superuser reading raw pages can, until the table is
-- rewritten. No statement updates a Slot row after the drop, so nothing else
-- clears them. Old row versions, and the dropped column's statistics rows in
-- pg_statistic, keep copies the same way; the WAL and any backup keep theirs.
-- Why the file is shaped this way: this crate's NODE.inf.json notes, under
-- this file.

-- No Slot or node is written while the check and the drop run.
LOCK TABLE commission_slot, node IN SHARE ROW EXCLUSIVE MODE;

-- Every Slot has its node: one that exists, a `slot` node whose segment is the
-- Slot's element id, under the `commission.slots` node whose parent is its own
-- commission's node. The Slot's title is that node's name. Each refusal names
-- an element id alone. Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT s.element_id INTO offender FROM commission_slot s
    WHERE s.node_id IS NULL
    ORDER BY s.element_id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the slot title drop stops: slot % has no node', offender;
    END IF;

    SELECT s.element_id INTO offender FROM commission_slot s
    WHERE NOT EXISTS (
        SELECT 1 FROM node n
        JOIN node slots ON slots.id = n.parent
        JOIN commission c ON c.node_id = slots.parent
        WHERE n.id = s.node_id
          AND n.type = 'slot'
          AND n.segment = s.element_id::text
          AND slots.type = 'commission.slots'
          AND c.id = s.commission_id
    )
    ORDER BY s.element_id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the slot title drop stops: slot % points at no node of its own', offender;
    END IF;

    SELECT s.element_id INTO offender FROM commission_slot s
    JOIN node n ON n.id = s.node_id
    WHERE s.title IS DISTINCT FROM n.name
    ORDER BY s.element_id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the slot title drop stops: slot % has a title its node''s name differs from', offender;
    END IF;
END
$$;

-- Every row has its own node, so node_id holds it from here on and the check
-- that stood in for that goes. The title's own CHECK goes with the column. No
-- CASCADE: whatever PostgreSQL records as depending on the dropped column, such
-- as a view or a column trigger, stops the migration instead of being dropped
-- with it.
ALTER TABLE commission_slot
    ALTER COLUMN node_id SET NOT NULL,
    DROP CONSTRAINT commission_slot_has_its_node,
    DROP COLUMN title;
