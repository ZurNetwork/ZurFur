-- Every Slot gets its node: a Slot declared since the node table came, with no
-- node, gets one under its commission's `slots` entry, named by its title,
-- private. From here on a slot node has its Slot, and a new or updated Slot
-- row has a node. Nothing is repaired: a slot node no Slot points at, a bound
-- Slot whose node's name differs from its title, and a Slot whose commission
-- has no `slots` entry each stop the fill, naming an id alone. Why the file is
-- shaped this way: this crate's NODE.inf.json notes, under this file.

-- No commission, Slot or node is written while the checks and the fill run.
LOCK TABLE commission, commission_slot, node IN SHARE ROW EXCLUSIVE MODE;

-- At commit, a commission node has the commission that points at it, and a
-- slot node the Slot that points at it. A node deleted since is not checked.
CREATE OR REPLACE FUNCTION node_has_its_thing() RETURNS trigger AS $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM node n WHERE n.id = NEW.id) THEN
        RETURN NULL;
    END IF;
    IF NEW.type = 'slot' THEN
        IF NOT EXISTS (SELECT 1 FROM commission_slot s WHERE s.node_id = NEW.id) THEN
            RAISE EXCEPTION 'node % has no slot: a slot node stands only with its slot',
                NEW.id;
        END IF;
        RETURN NULL;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM commission c WHERE c.node_id = NEW.id) THEN
        RAISE EXCEPTION 'node % has no commission: a commission node stands only with its commission',
            NEW.id;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER node_has_its_thing ON node;

CREATE CONSTRAINT TRIGGER node_has_its_thing
    AFTER INSERT ON node
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    WHEN (NEW.type IN ('commission', 'slot'))
    EXECUTE FUNCTION node_has_its_thing();

-- A new or updated Slot row has a node. Not validated: the rows the fill below
-- binds are the only ones without.
ALTER TABLE commission_slot ADD CONSTRAINT commission_slot_has_its_node CHECK (node_id IS NOT NULL) NOT VALID;

-- Everything the fill reads must be readable before any row is written. Each
-- refusal names an id alone. Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT n.id INTO offender FROM node n
    WHERE n.type = 'slot'
      AND NOT EXISTS (SELECT 1 FROM commission_slot s WHERE s.node_id = n.id)
    ORDER BY n.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the slot node fill stops: node % is a slot node no slot points at', offender;
    END IF;

    SELECT s.element_id INTO offender FROM commission_slot s
    JOIN node n ON n.id = s.node_id
    WHERE n.name IS DISTINCT FROM s.title
    ORDER BY s.element_id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the slot node fill stops: slot % has a title its node''s name differs from', offender;
    END IF;

    SELECT s.element_id INTO offender FROM commission_slot s
    WHERE NOT EXISTS (
        SELECT 1 FROM commission c
        JOIN node slots ON slots.parent = c.node_id AND slots.type = 'commission.slots'
        WHERE c.id = s.commission_id
    )
    ORDER BY s.element_id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the slot node fill stops: slot % has no slots entry to sit under', offender;
    END IF;
END
$$;

-- Each Slot without a node gets one under its commission's `slots` entry: its
-- element id as the segment, its title as the name, private. Only the nodes
-- this statement inserts are bound.
WITH made AS (
    INSERT INTO node (id, parent, segment, type, level, name)
    SELECT gen_random_uuid(), slots.id, s.element_id::text, 'slot', 'private', s.title
    FROM commission_slot s
    JOIN commission c ON c.id = s.commission_id
    JOIN node slots ON slots.parent = c.node_id AND slots.type = 'commission.slots'
    WHERE s.node_id IS NULL
    RETURNING id, segment
)
UPDATE commission_slot s
SET node_id = made.id
FROM made
WHERE made.segment = s.element_id::text;
