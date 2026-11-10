-- The Den's nodes: each commission, its six fixed entries and its Slots, as a
-- tree. A commission and each of its Slots point at their own node, and a node
-- never outlives the row that points at it. Then every existing commission and
-- Slot gets its nodes. Why the file is shaped this way: this crate's NODE.json
-- notes, under this file.
--
-- node.id       App-minted. The rows this migration writes get random (v4) ids.
-- node.parent   The node it sits under. NULL only for a commission's node, which
--               sits directly under a top-level directory. Deleting a node takes
--               every node under it.
-- node.segment  Its path segment: 1 to 2,048 bytes, each one of A-Z, a-z, 0-9
--               and `._:%~-`, and never `.` or `..`. Unique under its parent;
--               two nodes with no parent never share one either. A commission's
--               is its id, a Slot's its element id, an entry's its word.
-- node.type     What it is: `commission`, one of the six entries
--               (`commission.attachments`, `.brief`, `.changelog`,
--               `.participants`, `.products`, `.slots`) or `slot`. An entry sits
--               under a commission, a Slot under a `commission.slots` entry.
-- node.level    How far it shows: private, listed or public.
-- node.name     What a person reads: never empty.
-- commission.node_id, commission_slot.node_id
--               The row's own node: NULL until it is set, then never changed.
--               Checked at commit against the row (type, segment, and for a
--               Slot its own commission's `slots` entry).
CREATE TABLE node (
    id      uuid PRIMARY KEY,
    parent  uuid REFERENCES node (id) ON DELETE CASCADE,
    segment text COLLATE "C" NOT NULL CONSTRAINT node_segment_shape CHECK (
                octet_length(segment) BETWEEN 1 AND 2048
                AND segment !~ '[^ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._:%~-]'
                AND segment NOT IN ('.', '..')),
    type    text NOT NULL CONSTRAINT node_type_known CHECK (type IN (
                'commission', 'commission.attachments', 'commission.brief', 'commission.changelog',
                'commission.participants', 'commission.products', 'commission.slots', 'slot')),
    level   text NOT NULL CONSTRAINT node_level_known CHECK (level IN ('private', 'listed', 'public')),
    name    text NOT NULL CONSTRAINT node_name_not_empty CHECK (name <> ''),
    CONSTRAINT node_parent_segment_key UNIQUE NULLS NOT DISTINCT (parent, segment),
    CONSTRAINT node_top_level_is_a_commission CHECK ((parent IS NULL) = (type = 'commission')),
    CONSTRAINT node_entry_segment_is_its_word CHECK (
                type NOT IN ('commission.attachments', 'commission.brief', 'commission.changelog',
                             'commission.participants', 'commission.products', 'commission.slots')
                OR type = 'commission.' || segment)
);

ALTER TABLE commission      ADD COLUMN node_id uuid UNIQUE REFERENCES node (id) DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE commission_slot ADD COLUMN node_id uuid UNIQUE REFERENCES node (id) DEFERRABLE INITIALLY DEFERRED;

-- A node is never moved, renamed in its path or retyped. Its name and level
-- may change.
CREATE FUNCTION node_keeps_its_place() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'node % keeps its place: its id, parent, segment and type never change', OLD.id;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER node_keeps_its_place
    BEFORE UPDATE OF id, parent, segment, type ON node
    FOR EACH ROW EXECUTE FUNCTION node_keeps_its_place();

-- An entry sits under a commission node and a Slot under a `commission.slots`
-- node. A parent this statement cannot see yet counts as the wrong type. A
-- node with no parent is left to node_top_level_is_a_commission.
CREATE FUNCTION node_fits_its_parent() RETURNS trigger AS $$
DECLARE
    parent_type text;
BEGIN
    IF NEW.parent IS NULL THEN
        RETURN NEW;
    END IF;
    SELECT n.type INTO parent_type FROM node n WHERE n.id = NEW.parent;
    IF NEW.type = 'slot' AND parent_type IS DISTINCT FROM 'commission.slots' THEN
        RAISE EXCEPTION 'node % does not fit under node %: a slot sits only under a commission.slots node',
            NEW.id, NEW.parent;
    END IF;
    IF NEW.type IN ('commission.attachments', 'commission.brief', 'commission.changelog',
                    'commission.participants', 'commission.products', 'commission.slots')
       AND parent_type IS DISTINCT FROM 'commission' THEN
        RAISE EXCEPTION 'node % does not fit under node %: an entry sits only under a commission node',
            NEW.id, NEW.parent;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER node_fits_its_parent
    BEFORE INSERT ON node
    FOR EACH ROW EXECUTE FUNCTION node_fits_its_parent();

-- A fixed entry leaves only with its commission: deleting one is refused while
-- its parent lives. The cascade from a deleted commission node runs after that
-- node is gone, so the probe finds nothing and lets it through.
CREATE FUNCTION node_entry_stays_with_its_commission() RETURNS trigger AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM node n WHERE n.id = OLD.parent) THEN
        RAISE EXCEPTION 'node % is a fixed entry of node %: it leaves only with its commission',
            OLD.id, OLD.parent;
    END IF;
    RETURN OLD;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER node_entry_stays_with_its_commission
    BEFORE DELETE ON node
    FOR EACH ROW
    WHEN (OLD.type IN ('commission.attachments', 'commission.brief', 'commission.changelog',
                       'commission.participants', 'commission.products', 'commission.slots'))
    EXECUTE FUNCTION node_entry_stays_with_its_commission();

-- node_id goes from NULL to a node once, and never to another node or back to
-- NULL.
CREATE FUNCTION node_id_is_set_once() RETURNS trigger AS $$
BEGIN
    IF OLD.node_id IS NOT NULL AND NEW.node_id IS DISTINCT FROM OLD.node_id THEN
        RAISE EXCEPTION 'a % row keeps node %: node_id is set once', TG_TABLE_NAME, OLD.node_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER node_id_is_set_once
    BEFORE UPDATE OF node_id ON commission
    FOR EACH ROW EXECUTE FUNCTION node_id_is_set_once();

CREATE TRIGGER node_id_is_set_once
    BEFORE UPDATE OF node_id ON commission_slot
    FOR EACH ROW EXECUTE FUNCTION node_id_is_set_once();

-- A deleted commission or Slot takes its node, and the node's own subtree,
-- with it. A row with no node deletes nothing.
CREATE FUNCTION node_follows_its_thing() RETURNS trigger AS $$
BEGIN
    DELETE FROM node WHERE id = OLD.node_id;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER node_follows_its_thing
    AFTER DELETE ON commission
    FOR EACH ROW EXECUTE FUNCTION node_follows_its_thing();

CREATE TRIGGER node_follows_its_thing
    AFTER DELETE ON commission_slot
    FOR EACH ROW EXECUTE FUNCTION node_follows_its_thing();

-- A truncated commission or Slot table takes every node of its type; the
-- parent key's cascade takes the entries and Slot nodes under a commission's.
CREATE FUNCTION node_follows_a_truncate() RETURNS trigger AS $$
BEGIN
    DELETE FROM node WHERE type = TG_ARGV[0];
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER node_follows_a_truncate
    AFTER TRUNCATE ON commission
    FOR EACH STATEMENT EXECUTE FUNCTION node_follows_a_truncate('commission');

CREATE TRIGGER node_follows_a_truncate
    AFTER TRUNCATE ON commission_slot
    FOR EACH STATEMENT EXECUTE FUNCTION node_follows_a_truncate('slot');

-- At commit, a row's node is its own: a commission's is a top-level
-- `commission` node whose segment is its id; a Slot's is a `slot` node whose
-- segment is its element id, under a `commission.slots` node whose parent is
-- its own commission's node. A row deleted since is not checked.
CREATE FUNCTION node_matches_its_thing() RETURNS trigger AS $$
DECLARE
    pointed uuid;
    owning_commission uuid;
BEGIN
    IF TG_TABLE_NAME = 'commission' THEN
        SELECT c.node_id INTO pointed FROM commission c WHERE c.id = NEW.id;
        IF NOT FOUND THEN
            RETURN NULL;
        END IF;
        IF NOT EXISTS (
            SELECT 1 FROM node n
            WHERE n.id = pointed
              AND n.type = 'commission'
              AND n.parent IS NULL
              AND n.segment = NEW.id::text
        ) THEN
            RAISE EXCEPTION 'commission % does not match node %: a commission points only at its own node',
                NEW.id, pointed;
        END IF;
        RETURN NULL;
    END IF;

    SELECT s.node_id, s.commission_id INTO pointed, owning_commission
    FROM commission_slot s WHERE s.element_id = NEW.element_id;
    IF NOT FOUND THEN
        RETURN NULL;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM node n
        JOIN node slots ON slots.id = n.parent
        JOIN commission c ON c.node_id = slots.parent
        WHERE n.id = pointed
          AND n.type = 'slot'
          AND n.segment = NEW.element_id::text
          AND slots.type = 'commission.slots'
          AND c.id = owning_commission
    ) THEN
        RAISE EXCEPTION 'slot % does not match node %: a slot points only at its own node under its own commission',
            NEW.element_id, pointed;
    END IF;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE CONSTRAINT TRIGGER node_matches_its_thing
    AFTER INSERT OR UPDATE OF node_id, id ON commission
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    WHEN (NEW.node_id IS NOT NULL)
    EXECUTE FUNCTION node_matches_its_thing();

CREATE CONSTRAINT TRIGGER node_matches_its_thing
    AFTER INSERT OR UPDATE OF node_id, element_id, commission_id ON commission_slot
    DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW
    WHEN (NEW.node_id IS NOT NULL)
    EXECUTE FUNCTION node_matches_its_thing();

-- Every commission must be readable before any node is written: a visibility
-- outside the three levels or an empty title stops the migration, naming the
-- commission by its id alone. Nothing is repaired.
DO $$
DECLARE
    offender uuid;
BEGIN
    SELECT c.id INTO offender FROM commission c
    WHERE c.visibility NOT IN ('private', 'listed', 'public')
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the node backfill stops: commission % has an unreadable visibility', offender;
    END IF;

    SELECT c.id INTO offender FROM commission c
    WHERE c.title = ''
    ORDER BY c.id
    LIMIT 1;
    IF FOUND THEN
        RAISE EXCEPTION 'the node backfill stops: commission % has an empty title', offender;
    END IF;
END
$$;

-- Each commission's node: its id as the segment, its visibility as the level,
-- its title as the name. Archived commissions too.
INSERT INTO node (id, parent, segment, type, level, name)
SELECT gen_random_uuid(), NULL, c.id::text, 'commission', c.visibility, c.title
FROM commission c;

UPDATE commission c
SET node_id = n.id
FROM node n
WHERE n.parent IS NULL
  AND n.type = 'commission'
  AND n.segment = c.id::text;

-- Each commission's six entries, private.
INSERT INTO node (id, parent, segment, type, level, name)
SELECT gen_random_uuid(), c.node_id, entry.word, 'commission.' || entry.word, 'private', entry.word
FROM commission c
CROSS JOIN (VALUES ('attachments'), ('products'), ('slots'), ('changelog'), ('participants'), ('brief'))
    AS entry (word);

-- Each Slot's node under its own commission's `slots` entry: its element id
-- as the segment, its title as the name, private.
INSERT INTO node (id, parent, segment, type, level, name)
SELECT gen_random_uuid(), slots.id, s.element_id::text, 'slot', 'private', s.title
FROM commission_slot s
JOIN commission c ON c.id = s.commission_id
JOIN node slots ON slots.parent = c.node_id AND slots.type = 'commission.slots';

UPDATE commission_slot s
SET node_id = n.id
FROM commission c, node slots, node n
WHERE c.id = s.commission_id
  AND slots.parent = c.node_id
  AND slots.type = 'commission.slots'
  AND n.parent = slots.id
  AND n.type = 'slot'
  AND n.segment = s.element_id::text;
